//! The input's kind, from its content and never from its name: a cabinet's signature is a
//! package, a OneStore header is a section or a notebook, and a directory is read through the
//! one .onetoc2 it holds. A ZIP, and anything else, is refused.
//!
//! Whether a OneStore file is a section or a notebook is onenote_parser's store type to say,
//! not its header's: a OneDrive download's .onetoc2 carries the section file type GUID in its
//! header ([MS-ONESTORE] 2.3.1's guidFileType), so the header alone can't tell them apart.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::Fault;

/// What the input is.
#[derive(Debug, PartialEq)]
pub enum Input {
    /// A OneStore file: a .one section or a .onetoc2 notebook, as the parser says.
    OneStore,
    /// A notebook directory, read through the .onetoc2 at this path.
    Directory(PathBuf),
    /// A cabinet: a .onepkg package.
    Package,
}

/// What a file's first bytes say it is.
#[derive(Debug, PartialEq)]
enum Content {
    OneStore,
    Cabinet,
    Zip,
    Other,
}

/// The bytes [`content`] needs: a OneStore header's first four GUIDs.
const PREFIX: usize = 64;

/// CFHEADER.signature, "MSCF" ([MS-CAB] 2.1).
const CAB_MAGIC: &[u8] = b"MSCF";
/// A ZIP's local file header, its end of central directory (an empty archive), and its
/// spanning marker (APPNOTE 4.3.7, 4.3.16, 8.5.3).
const ZIP_MAGICS: [&[u8]; 3] = [b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08"];
/// guidFileFormat, at offset 48 of a OneStore header ([MS-ONESTORE] 2.3.1): the revision store
/// file format {109ADD3F-911B-49F5-A5D0-1791EDC8AED8} of desktop files, and the packaging
/// format {638DE92F-A6D4-4BC1-9A36-B3FC2511A5B7} of OneDrive downloads (2.8.1), each in its
/// stored byte order. These are the formats onenote_parser reads.
const ONESTORE_FORMATS: [[u8; 16]; 2] = [
    [
        0x3f, 0xdd, 0x9a, 0x10, 0x1b, 0x91, 0xf5, 0x49, 0xa5, 0xd0, 0x17, 0x91, 0xed, 0xc8, 0xae,
        0xd8,
    ],
    [
        0x2f, 0xe9, 0x8d, 0x63, 0xd4, 0xa6, 0xc1, 0x4b, 0x9a, 0x36, 0xb3, 0xfc, 0x25, 0x11, 0xa5,
        0xb7,
    ],
];

/// The input at `path`, or the refusal of it.
pub fn sniff(path: &Path) -> Result<Input, Fault> {
    let refuse = |reason: String| Fault::Refused(format!("{}: {reason}", path.display()));
    let metadata = fs::metadata(path).map_err(|err| refuse(err.to_string()))?;
    if metadata.is_dir() {
        return toc_in(path).map(Input::Directory).map_err(refuse);
    }
    let mut prefix = Vec::with_capacity(PREFIX);
    File::open(path)
        .and_then(|file| file.take(PREFIX as u64).read_to_end(&mut prefix))
        .map_err(|err| refuse(err.to_string()))?;
    match content(&prefix) {
        Content::OneStore => Ok(Input::OneStore),
        Content::Cabinet => Ok(Input::Package),
        Content::Zip => Err(refuse(
            "a ZIP archive; extract the OneDrive download first".to_owned(),
        )),
        Content::Other => Err(refuse(
            "not a OneNote section, notebook, or package".to_owned(),
        )),
    }
}

fn content(prefix: &[u8]) -> Content {
    if prefix.starts_with(CAB_MAGIC) {
        return Content::Cabinet;
    }
    if ZIP_MAGICS.iter().any(|magic| prefix.starts_with(magic)) {
        return Content::Zip;
    }
    if prefix.len() == PREFIX
        && ONESTORE_FORMATS
            .iter()
            .any(|format| prefix[48..] == *format)
    {
        return Content::OneStore;
    }
    Content::Other
}

/// The one .onetoc2 in `dir`, told by its name, as OneNote names a notebook's table of
/// contents; or why there isn't one. A section group's .onetoc2, in a directory below, is the
/// notebook's to find.
fn toc_in(dir: &Path) -> Result<PathBuf, String> {
    let mut tocs = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let path = entry.map_err(|err| err.to_string())?.path();
        let is_toc = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("onetoc2"));
        if is_toc && path.is_file() {
            tocs.push(path);
        }
    }
    tocs.sort_unstable();
    match tocs.len() {
        0 => Err("no .onetoc2 in the directory".to_owned()),
        1 => Ok(tocs.remove(0)),
        _ => {
            let names: Vec<String> = tocs
                .iter()
                .map(|toc| toc.file_name().unwrap_or_default().to_string_lossy().into())
                .collect();
            Err(format!(
                "more than one .onetoc2 in the directory: {}",
                names.join(", ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A OneStore header's first 64 bytes, with the given guidFileFormat.
    fn header(format: &[u8; 16]) -> Vec<u8> {
        let mut bytes = vec![0; 48];
        bytes.extend_from_slice(format);
        bytes
    }

    #[test]
    fn a_short_or_unknown_header_is_other() {
        assert_eq!(content(&header(&[0; 16])), Content::Other);
        assert_eq!(content(&header(&ONESTORE_FORMATS[0])[..63]), Content::Other);
        assert_eq!(content(b""), Content::Other);
    }

    #[test]
    fn cabinets_and_zips_are_told_by_their_signatures() {
        assert_eq!(content(b"MSCF\0\0\0\0"), Content::Cabinet);
        for magic in [b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08"] {
            assert_eq!(content(magic), Content::Zip);
        }
    }
}
