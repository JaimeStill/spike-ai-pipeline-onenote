//! The lines of schema "1". Each line is one JSON object whose "line" field names it: a notebook
//! line first, then section lines, each followed by its page lines. Fields serialize in
//! declaration order, and a field with no value, or an empty one, is left out, never written as
//! null.

use serde::Serialize;

/// The schema every line of this output follows.
pub const SCHEMA: &str = "1";

/// The first line: what the input is.
#[derive(Serialize)]
pub struct Notebook {
    pub line: &'static str,
    pub schema: &'static str,
    /// "section", "notebook" or "package".
    pub input: &'static str,
    /// The notebook directory's name, the package's, or the section file's stem.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// For a notebook or a package, its .onetoc2's FileIdentity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// For a notebook or a package, its colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One section, written before its pages.
#[derive(Serialize)]
pub struct Section {
    pub line: &'static str,
    /// The section's FileIdentity.
    pub id: String,
    /// The id of the section group the section is in; a section at the notebook's root, or a
    /// section input, has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The display names from the notebook's root down to this section.
    pub path: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One page, at its latest revision.
#[derive(Serialize)]
pub struct Page {
    pub line: &'static str,
    /// The page's GUID.
    pub id: String,
    /// The id of the section the page is in.
    pub section: String,
    /// The page's level in its section's page tree: 1 for a page, 2 for its subpage, and so on.
    pub level: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// ISO 8601 UTC, to the second, as is the modification time.
    pub creation_time: String,
    pub modification_time: String,
    /// The author, as stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}
