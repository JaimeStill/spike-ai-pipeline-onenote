//! The media directory. Each distinct content of an image or an embedded file is one file there,
//! named by the lowercase hex SHA-256 of its bytes and, when the stored extension is 1 to 8
//! ASCII letters and digits, "." and that extension. A stored filename is never part of a path.
//! Without --media the names are worked out the same way and nothing is written.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::Fault;

pub struct Media {
    /// The --media directory; none without --media.
    dir: Option<PathBuf>,
    /// Whether the directory has been created, which it is before its first file.
    created: bool,
    /// The names already written, each content's once.
    written: HashSet<String>,
}

impl Media {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            created: false,
            written: HashSet::new(),
        }
    }

    /// Names a content, and with --media writes it, unless the same content was written
    /// already. A directory or file that can't be written is a fault: exit 1.
    pub fn store(&mut self, bytes: &[u8], extension: Option<&str>) -> Result<String, Fault> {
        let name = name(bytes, extension);
        let Some(dir) = &self.dir else {
            return Ok(name);
        };
        if self.written.contains(&name) {
            return Ok(name);
        }
        if !self.created {
            fs::create_dir_all(dir).map_err(|err| {
                Fault::Media(format!(
                    "media directory {} can't be created: {err}",
                    dir.display()
                ))
            })?;
            self.created = true;
        }
        let path = dir.join(&name);
        fs::write(&path, bytes).map_err(|err| {
            Fault::Media(format!(
                "media file {} can't be written: {err}",
                path.display()
            ))
        })?;
        self.written.insert(name.clone());
        Ok(name)
    }
}

/// A content's file name: its SHA-256 in lowercase hex, with the extension when it is usable.
fn name(bytes: &[u8], extension: Option<&str>) -> String {
    let mut name = String::with_capacity(64 + 9);
    for byte in Sha256::digest(bytes) {
        write!(name, "{byte:02x}").expect("writing to a String can't fail");
    }
    if let Some(extension) = extension.filter(|ext| usable(ext)) {
        name.push('.');
        name.push_str(extension);
    }
    name
}

/// Whether an extension may end a file name: 1 to 8 ASCII letters and digits, which keeps a
/// stored value from adding a separator, a dot, or anything else to the path.
fn usable(extension: &str) -> bool {
    (1..=8).contains(&extension.len()) && extension.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A stored filename's extension: what follows its last dot, if anything does.
pub fn extension_of(filename: &str) -> Option<&str> {
    filename
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 of the empty string, and of "abc", as FIPS 180-2 gives it.
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn names_are_the_sha256_and_a_usable_extension() {
        assert_eq!(name(b"", None), EMPTY);
        assert_eq!(name(b"abc", Some("png")), format!("{ABC}.png"));
        assert_eq!(name(b"abc", Some("JPEG2000")), format!("{ABC}.JPEG2000"));
        for unusable in ["", "jpeg20001", "tar.gz", "../x", "p/ng", "pñg", "a b"] {
            assert_eq!(name(b"abc", Some(unusable)), ABC, "{unusable:?}");
        }
    }

    #[test]
    fn a_filenames_extension_follows_its_last_dot() {
        assert_eq!(extension_of("report.final.docx"), Some("docx"));
        assert_eq!(extension_of("README"), None);
        assert_eq!(extension_of("trailing."), None);
        // Whatever follows the last dot, even a path; the name keeps it only when usable.
        assert_eq!(extension_of("a.b/c"), Some("b/c"));
    }

    /// A content stored twice is written once: the file removed after the first store is not
    /// written again by the second, which still names it.
    #[test]
    fn a_repeated_content_is_written_once() {
        let dir = std::env::temp_dir().join(format!("onenote-media-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut media = Media::new(Some(dir.join("made")));
        let first = media.store(b"abc", Some("txt")).ok().unwrap();
        let path = dir.join("made").join(&first);
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        fs::remove_file(&path).unwrap();
        let second = media.store(b"abc", Some("txt")).ok().unwrap();
        assert_eq!(second, first);
        assert!(!path.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_directory_nothing_is_written() {
        let mut media = Media::new(None);
        assert_eq!(media.store(b"", None).ok(), Some(EMPTY.to_owned()));
        assert!(media.written.is_empty());
    }
}
