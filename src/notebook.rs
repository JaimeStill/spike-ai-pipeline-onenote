//! A notebook: a .onetoc2, a notebook directory read through its .onetoc2, or a .onepkg
//! package. Its notebook line comes first, then its section groups and sections depth-first in
//! the table of contents' order, each section followed by its pages, then a skipped line for
//! each section the parser couldn't read.
//!
//! onenote_parser drops three kinds of entry from a notebook, and the CLI says what it can of
//! each:
//! - a section that fails to parse it reports as a notebook warning, which the CLI turns into a
//!   skipped line and a warning. The parser keeps no place for it among the entries, so the
//!   skipped lines follow them. A section that fails inside a section group isn't reported at
//!   all: the parser discards the group's report.
//! - the recycle bin it drops without a word, so the CLI looks for it itself: a
//!   OneNote_RecycleBin entry beside the .onetoc2, in the directory or in the cabinet.
//! - an entry whose file is missing it drops without a word too, and the CLI can't tell: the
//!   table of contents' entries aren't public, and a package's file system, where they could
//!   be watched, is the parser's own.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use onenote_parser::Parser;
use onenote_parser::notebook::Notebook;
use onenote_parser::section::SectionEntry;

use crate::guard::guarded;
use crate::jsonl::Lines;
use crate::media::Media;
use crate::model::{self, SCHEMA};
use crate::section::{self, hex, place, typed};
use crate::{Fault, warn};

/// The directory OneNote keeps a notebook's deleted sections and pages in, beside its .onetoc2.
const RECYCLE_BIN: &str = "OneNote_RecycleBin";

/// The start of the notebook warning onenote_parser gives a section it couldn't read: "failed
/// to import section <path>: <reason>", the path its .onetoc2's directory joined with the
/// entry's name.
const FAILED_SECTION: &str = "failed to import section ";

/// Reads the notebook whose table of contents is the .onetoc2 at `toc`, named for the directory
/// it is in.
pub fn read(toc: &Path, media: &mut Media, out: &mut Lines<impl Write>) -> Result<(), Fault> {
    let notebook = guarded(|| Parser::new().parse_notebook(typed(toc)))
        .map_err(|failure| refused(toc, &failure.reason()))?;
    let toc_dir = toc.parent().unwrap_or(Path::new(""));
    let dir = if toc_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        toc_dir
    };
    let read = Read {
        input: "notebook",
        name: dir_name(dir),
        // The parser joins the entries' names to its path's parent, as given.
        toc_dir: typed(toc_dir).to_string_lossy().into_owned(),
        recycle_bin: dir.join(RECYCLE_BIN).exists(),
    };
    read.write(&notebook, media, out)
}

/// Reads the .onepkg package at `path`, named for its file's stem.
pub fn read_package(
    path: &Path,
    media: &mut Media,
    out: &mut Lines<impl Write>,
) -> Result<(), Fault> {
    let notebook = guarded(|| Parser::new().parse_package(typed(path)))
        .map_err(|failure| refused(path, &failure.reason()))?;
    // The parser read the cabinet, so a second reading of its entries fails only if the file
    // changed in between.
    let names = File::open(path)
        .and_then(cab::Cabinet::new)
        .map(|cabinet| {
            cabinet
                .folder_entries()
                .flat_map(|folder| folder.file_entries())
                .map(|entry| entry.name().to_owned())
                .collect::<Vec<_>>()
        })
        .map_err(|err| refused(path, &format!("its cabinet can't be listed: {err}")))?;
    let entries: Vec<Vec<&str>> = names.iter().map(|name| components(name)).collect();
    let toc_dir = package_toc_dir(&entries).unwrap_or_default();
    let read = Read {
        input: "package",
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        toc_dir: toc_dir.join("/"),
        recycle_bin: holds_recycle_bin(&entries, &toc_dir),
    };
    read.write(&notebook, media, out)
}

/// What a notebook's lines need beyond what the parser read.
struct Read {
    /// The notebook line's input kind: "notebook" or "package".
    input: &'static str,
    name: String,
    /// The directory of the .onetoc2, as the parser names the sections beside it.
    toc_dir: String,
    /// Whether a recycle bin is beside the .onetoc2.
    recycle_bin: bool,
}

impl Read {
    fn write(
        &self,
        notebook: &Notebook,
        media: &mut Media,
        out: &mut Lines<impl Write>,
    ) -> Result<(), Fault> {
        out.write(&model::Notebook {
            line: "notebook",
            schema: SCHEMA,
            input: self.input,
            name: self.name.clone(),
            id: Some(notebook.file_identity().to_string()),
            color: notebook.color().and_then(hex),
        })
        .map_err(Fault::Output)?;
        if self.recycle_bin {
            warn(
                &place(&[RECYCLE_BIN.to_owned()]),
                "the recycle bin is skipped; its sections and pages are not read",
            );
        }
        entries(notebook.entries(), None, &[], media, out)?;
        for warning in notebook.report().warnings() {
            match failed_section(warning.message(), &self.toc_dir) {
                Some((name, reason)) => {
                    warn(
                        &place(&[name.to_owned()]),
                        &format!("the section is skipped: {reason}"),
                    );
                    out.write(&model::Skipped {
                        line: "skipped",
                        kind: "section",
                        name: name.to_owned(),
                        reason: reason.to_owned(),
                    })
                    .map_err(Fault::Output)?;
                }
                None => warn("/", warning.message()),
            }
        }
        Ok(())
    }
}

/// Writes `entries` depth-first, in the table of contents' order, under the group `parent` at
/// the names `above`.
fn entries(
    entries: &[SectionEntry],
    parent: Option<&str>,
    above: &[String],
    media: &mut Media,
    out: &mut Lines<impl Write>,
) -> Result<(), Fault> {
    for entry in entries {
        match entry {
            SectionEntry::Section(section) => section::write(section, parent, above, media, out)?,
            SectionEntry::SectionGroup(group) => {
                let id = group.file_identity().to_string();
                let mut path = above.to_vec();
                path.push(group.display_name().to_owned());
                out.write(&model::SectionGroup {
                    line: "section_group",
                    id: id.clone(),
                    parent: parent.map(str::to_owned),
                    path: path.clone(),
                    display_name: group.display_name().to_owned(),
                })
                .map_err(Fault::Output)?;
                self::entries(group.entries(), Some(&id), &path, media, out)?;
            }
        }
    }
    Ok(())
}

/// The section name and reason of the parser's warning for a section it couldn't read, which
/// names the section by its path: `toc_dir`, then the entry's name. That name is sanitised, so
/// it holds no separator and no colon, and the first ": " after it starts the reason.
fn failed_section<'a>(message: &'a str, toc_dir: &str) -> Option<(&'a str, &'a str)> {
    let mut entry = message
        .strip_prefix(FAILED_SECTION)?
        .strip_prefix(toc_dir)?;
    if !toc_dir.is_empty() && !toc_dir.ends_with('/') {
        entry = entry.strip_prefix('/')?;
    }
    entry
        .split_once(": ")
        .filter(|(name, _)| !name.is_empty() && !name.contains('/'))
}

/// A directory's name; for "." or "..", the name of the directory it is.
fn dir_name(dir: &Path) -> String {
    let named = match dir.file_name() {
        Some(name) => Some(name.to_owned()),
        None => dir
            .canonicalize()
            .ok()
            .and_then(|dir| dir.file_name().map(ToOwned::to_owned)),
    };
    named.unwrap_or_default().to_string_lossy().into_owned()
}

/// A cabinet entry's name as path components, as onenote_parser normalises it: either
/// separator, "." dropped, and ".." taking back the component before it.
fn components(name: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts
}

/// The directory of the .onetoc2 onenote_parser reads a package through: the first of the
/// shallowest, as the parser picks it.
fn package_toc_dir<'a>(entries: &[Vec<&'a str>]) -> Option<Vec<&'a str>> {
    let mut toc: Option<&Vec<&str>> = None;
    for entry in entries {
        let is_toc = entry
            .last()
            .and_then(|name| name.rsplit_once('.'))
            .is_some_and(|(_, ext)| ext == "onetoc2");
        if is_toc && toc.is_none_or(|toc| entry.len() < toc.len()) {
            toc = Some(entry);
        }
    }
    toc.map(|toc| toc[..toc.len() - 1].to_vec())
}

/// Whether a package's entries hold a file in a recycle bin beside its .onetoc2.
fn holds_recycle_bin(entries: &[Vec<&str>], toc_dir: &[&str]) -> bool {
    entries.iter().any(|entry| {
        entry.len() > toc_dir.len() + 1
            && entry.starts_with(toc_dir)
            && entry[toc_dir.len()] == RECYCLE_BIN
    })
}

/// The refusal of a notebook or a package the parser couldn't read.
fn refused(path: &Path, reason: &str) -> Fault {
    Fault::Refused(format!("{}: {reason}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_section_warning_gives_its_name_and_reason() {
        let message = "failed to import section /n/b: k/Encryption.one: Unexpected end of file";
        assert_eq!(
            failed_section(message, "/n/b: k"),
            Some(("Encryption.one", "Unexpected end of file"))
        );
        // A package's .onetoc2 at the cabinet's root names its sections bare.
        assert_eq!(
            failed_section("failed to import section A.one: a: b", ""),
            Some(("A.one", "a: b"))
        );
        assert_eq!(failed_section("something else", ""), None);
        assert_eq!(
            failed_section("failed to import section /nb/A.one: x", "/n"),
            None
        );
        assert_eq!(
            failed_section("failed to import section /A.one: x", "/"),
            Some(("A.one", "x"))
        );
    }

    #[test]
    fn a_packages_toc_is_its_first_shallowest() {
        let names = [
            "Group\\Open Notebook.onetoc2",
            "nb/Open Notebook.onetoc2",
            "nb/./x/../Other.onetoc2",
            "nb/A.one",
        ];
        let entries: Vec<Vec<&str>> = names.iter().map(|name| components(name)).collect();
        assert_eq!(entries[2], ["nb", "Other.onetoc2"]);
        assert_eq!(package_toc_dir(&entries), Some(vec!["Group"]));
        assert_eq!(package_toc_dir(&entries[3..]), None);
    }

    #[test]
    fn a_recycle_bin_counts_beside_the_toc_only() {
        let entries = |names: &[&'static str]| -> Vec<Vec<&'static str>> {
            names.iter().map(|name| components(name)).collect()
        };
        let beside = entries(&["OneNote_RecycleBin\\OneNote_DeletedPages.one"]);
        assert!(holds_recycle_bin(&beside, &[]));
        assert!(!holds_recycle_bin(&beside, &["nb"]));
        let deeper = entries(&["Group/OneNote_RecycleBin/OneNote_DeletedPages.one"]);
        assert!(!holds_recycle_bin(&deeper, &[]));
        // A section file by the name, not a directory, isn't a recycle bin.
        assert!(!holds_recycle_bin(&entries(&["OneNote_RecycleBin"]), &[]));
    }
}
