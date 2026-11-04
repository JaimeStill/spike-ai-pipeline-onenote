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
    /// The page's containers, its title first, then the rest in stored order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Container>,
}

/// A container on a page, its "kind" naming it. Offsets are from the page's origin, in
/// half-inch units, as stored; a container without one has none.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Container {
    /// The page's title area: its outlines' items, in order.
    Title {
        #[serde(flatten)]
        offsets: Offsets,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        items: Vec<Item>,
    },
    Outline {
        #[serde(flatten)]
        offsets: Offsets,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        items: Vec<Item>,
    },
    // Images, embedded files and ink carry their offsets alone until they're read.
    Image(Offsets),
    EmbeddedFile(Offsets),
    Ink(Offsets),
}

#[derive(Serialize)]
pub struct Offsets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_horizontal: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_vertical: Option<f32>,
}

/// An entry of an outline, nested as stored.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// An outline element: its blocks, then the items indented below it.
    Element {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        blocks: Vec<Block>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        children: Vec<Item>,
    },
    /// An outline group: items OneNote keeps together at their own indentation.
    Group {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        items: Vec<Item>,
    },
}

/// One block of an outline element, its "kind" naming it.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    // Images, embedded files and ink inside an outline are named, not yet read.
    Image,
    EmbeddedFile,
    Ink,
}

#[derive(Serialize)]
pub struct Paragraph {
    /// The paragraph style's id as stored: "p", "h1" to "h6", "PageTitle", "Quote", "code"...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The list marker of the outline element, on its first paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<List>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    /// The visible runs, in order; hidden runs are left out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<Run>,
}

/// A list marker: a bullet, or a number with its format and the text around it.
#[derive(Serialize)]
pub struct List {
    /// The bullet character, as stored in the list's font.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullet: Option<String>,
    /// The numbering format: "decimal", "lower-letter", "upper-roman"...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// The number the list restarts at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<i32>,
}

/// A note tag.
#[derive(Serialize)]
pub struct Tag {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The tag's icon, by onenote_parser's NoteTagShape name: "BlueCheckBox", "YellowStar"...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// "completed" or "open", as the tag's action item status says.
    pub status: &'static str,
    /// When a completed tag was completed: ISO 8601 UTC, to the second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_time: Option<String>,
}

/// A table: its rows of cells, in order.
#[derive(Serialize)]
pub struct Table {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    pub rows: Vec<Vec<Cell>>,
}

/// A table cell: its outline elements, as an outline's items.
#[derive(Serialize)]
pub struct Cell {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
}

/// A run of text with one formatting. A flag that isn't set, and a colour that is automatic,
/// are left out.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub text: String,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub strikethrough: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub superscript: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub subscript: bool,
    /// "#rrggbb".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    /// "#rrggbb".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    /// The link's target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

fn is_false(flag: &bool) -> bool {
    !flag
}
