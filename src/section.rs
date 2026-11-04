//! A section: its section line and its page lines, read through onenote_parser at each page's
//! latest revision, the only one the parser reads.

use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use onenote_parser::Parser;
use onenote_parser::errors::Error;
use onenote_parser::page::Page;
use onenote_parser::property::common::Color;
use typed_path::{PathType, TypedPath};

use crate::content;
use crate::guard::{Failure, guarded};
use crate::jsonl::Lines;
use crate::media::Media;
use crate::model::{self, SCHEMA};
use crate::{Fault, notebook_unread};

/// Reads a OneStore input: a section's lines, or, when the parser finds a table of contents,
/// the input handed on as a notebook.
pub fn read(path: &Path, media: &mut Media, out: &mut Lines<impl Write>) -> Result<(), Fault> {
    let parser = Parser::new();
    let section = match guarded(|| parser.parse_section(typed(path))) {
        Ok(section) => section,
        Err(Failure::Error(err)) if is_toc(&err) => return Err(notebook_unread(path)),
        Err(failure) => {
            return Err(Fault::Refused(format!(
                "{}: {}",
                path.display(),
                failure.reason()
            )));
        }
    };
    let stem = path.file_stem().unwrap_or_default();
    out.write(&model::Notebook {
        line: "notebook",
        schema: SCHEMA,
        input: "section",
        name: stem.to_string_lossy().into_owned(),
        id: None,
        color: None,
    })
    .map_err(Fault::Output)?;
    write(&section, None, &[], media, out)
}

/// Writes a section's line, under the group `parent` at the names `above`, then its pages in
/// the order of its page series, each page's media stored before its line.
pub fn write(
    section: &onenote_parser::section::Section,
    parent: Option<&str>,
    above: &[String],
    media: &mut Media,
    out: &mut Lines<impl Write>,
) -> Result<(), Fault> {
    let id = section.file_identity().to_string();
    let mut path = above.to_vec();
    path.push(section.display_name().to_owned());
    let place = place(&path);
    out.write(&model::Section {
        line: "section",
        id: id.clone(),
        parent: parent.map(str::to_owned),
        path,
        display_name: section.display_name().to_owned(),
        color: section.color().and_then(hex),
    })
    .map_err(Fault::Output)?;
    for page in section
        .page_series()
        .iter()
        .flat_map(|series| series.pages())
    {
        let line = page_line(page, &id, &place, media)?;
        out.write(&line).map_err(Fault::Output)?;
    }
    Ok(())
}

/// A page's line, its media stored first. `place` names its section in warnings.
fn page_line(
    page: &Page,
    section: &str,
    place: &str,
    media: &mut Media,
) -> Result<model::Page, Fault> {
    let text = |value: Option<&str>| value.filter(|text| !text.is_empty()).map(str::to_owned);
    let id = guid(page.link_target_id());
    let content = content::page(page, media, &format!("{place}, page {id}"))?;
    Ok(model::Page {
        line: "page",
        id,
        section: section.to_owned(),
        level: page.level(),
        title: text(page.title_text()),
        creation_time: iso8601(page.created_time()),
        modification_time: iso8601(page.updated_time()),
        author: text(page.author()),
        content,
    })
}

/// A section's place in a warning: its path from the notebook's root, "/Group/Section", with
/// control characters escaped so the warning stays one line.
fn place(path: &[String]) -> String {
    let mut place = String::new();
    for name in path {
        place.push('/');
        for c in name.chars() {
            if c.is_control() {
                place.extend(c.escape_debug());
            } else {
                place.push(c);
            }
        }
    }
    place
}

/// Whether the parser refused a section because its store is a table of contents. Its error
/// kind isn't public, so this reads ErrorKind::NotASectionFile's message.
fn is_toc(err: &Error) -> bool {
    err.to_string().starts_with("Not a section file: ")
}

/// A host path as onenote_parser takes it, from its bytes, so a path that isn't UTF-8 reads.
fn typed(path: &Path) -> TypedPath<'_> {
    TypedPath::new(path.as_os_str().as_bytes(), PathType::Unix)
}

/// A page GUID as the parser gives it, "{5EAF76B1-8DDE-...}", in FileIdentity's form:
/// lowercase and hyphenated, without braces.
fn guid(stored: &str) -> String {
    stored
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_ascii_lowercase()
}

/// A colour as "#rrggbb". The parser turns MS-ONE 2.2.7's stored top byte into an alpha of
/// 255 less it, so an alpha of 0 is the stored 0xFF of a COLORREF's "no colour", and has none.
fn hex(color: Color) -> Option<String> {
    (color.alpha() != 0).then(|| format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b()))
}

/// A time as ISO 8601 UTC, to the second; a fraction of a second is cut, not rounded.
pub fn iso8601(time: time::UtcDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::utc_datetime;

    #[test]
    fn times_are_cut_to_the_second() {
        assert_eq!(
            iso8601(utc_datetime!(2020-11-02 13:45:49.897)),
            "2020-11-02T13:45:49Z"
        );
    }

    #[test]
    fn a_place_is_the_sections_path_on_one_line() {
        let path = ["Group".to_owned(), "Sec\ntion".to_owned()];
        assert_eq!(place(&path), "/Group/Sec\\ntion");
    }

    #[test]
    fn page_guids_take_file_identity_form() {
        assert_eq!(
            guid("{5EAF76B1-8DDE-8745-A8A7-1A9865CC478A}"),
            "5eaf76b1-8dde-8745-a8a7-1a9865cc478a"
        );
    }
}
