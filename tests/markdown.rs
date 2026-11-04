//! A page line's markdown, read from the public fixtures. The test page of "New Section 1.one"
//! is written out in full, by hand from its model and the rendering rules, not from the CLI's
//! output; the other pages are checked for their Markdown's shape.

mod common;

use serde_json::Value;

use common::{fixture, run};

const LOREM: &str = "Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy \
                     eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam \
                     voluptua.";

/// A section's page lines, in order.
fn pages(file: &str) -> Vec<Value> {
    let out = run(&[fixture(file)]);
    assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    out.lines()
        .into_iter()
        .filter(|line| line["line"] == "page")
        .collect()
}

fn markdown(page: &Value) -> &str {
    page["markdown"].as_str().unwrap_or_default()
}

/// The test page's Markdown, but for its ink line, whose file is `ink`'s. The title area's
/// date and time are left out; the blank paragraphs are; the paragraphs indented under blank
/// ones, which Markdown can't indent, sit at the outline's level; the fourth numbered item
/// restarts at 1, so a comment parts it from the list before; underline, subscript,
/// superscript, highlight and colour are plain text. The page's containers go title, main
/// outline (vertical 2.4), ink (its strokes at 38643 ink units, 30.4 half inches) and a last,
/// empty outline (32.1).
fn test_page(ink: &str) -> String {
    let lorem3 = [LOREM; 3];
    [
        "# Test Page",
        "ABCDEF",
        "## ABCDEF",
        "- [ ] ABCDEF\n- [x] ABCDEFG",
        "[Important] ABCDEFGH",
        "**Lorem** ipsum *dolor* sit amet, consetetur ~~sadipscing~~ elitr, sed diam nonumy \
         eirmod tempor invidunt ut labore et dolore [magna](https://example.com) aliquyam \
         erat, sed diam voluptua.",
        LOREM,
        LOREM,
        "[http://example.com/](http://example.com/)",
        &format!("- {}\n  - {}\n    - {}", lorem3[0], lorem3[1], lorem3[2]),
        &format!(
            "1. {LOREM}\n   1. {LOREM}\n      1. {LOREM}\n2. {LOREM}\n3. {LOREM}\n\n<!-- -->\n\n1. {LOREM}"
        ),
        LOREM,
        LOREM,
        "| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |",
        "| A | B |\n| --- | --- |",
        "$$a=b$$",
        "![example images from TESTIMAGES archive]\
         (d6d4898c203cbff35fe92e844bbf404064293314a71d1e6b24e907334e5bdff9.jpg)",
        ink,
    ]
    .join("\n\n")
}

/// The page's ink file, as its page line names it.
fn ink_file(page: &Value) -> &str {
    page["ink"]["file"].as_str().expect("the page has ink")
}

#[test]
fn the_test_page_renders_in_full() {
    let pages = pages("onenote-rs/New Section 1.one");
    let ink = format!("![ink]({})", ink_file(&pages[0]));
    assert_eq!(markdown(&pages[0]), test_page(&ink));
}

/// handwriting_recognition.one's first page is the test page, its ink's alt text the page's
/// recognised words, two lines of "Hello World".
#[test]
fn recognised_ink_is_the_ink_lines_alt_text() {
    let pages = pages("onenote-rs/handwriting_recognition.one");
    let ink = format!("![ink: Hello World Hello World]({})", ink_file(&pages[0]));
    assert_eq!(markdown(&pages[0]), test_page(&ink));
    assert_eq!(markdown(&pages[1]), "# Test Page");
}

/// The other sections' pages, each Markdown checked for its shape: a title heading where the
/// page has a title, its text, media by their files, and none for a page with nothing on it.
#[test]
fn the_other_pages_render_their_titles_text_and_media() {
    let alternative = pages("interop/Resources/AlternativePackaging.one");
    // A page-title paragraph outside the title area is text: the title renders once.
    assert_eq!(
        markdown(&alternative[0]),
        "# Test Suite Sample\n\nTest Suite Sample"
    );

    let without = pages("interop/SUT/OneWithoutFileData.one");
    assert_eq!(
        markdown(&without[0]),
        "# The OneNote file without file data\n\n\
         This is a test file which does not contain the file data."
    );

    let with = &pages("interop/SUT/OneWithFileData.one")[0];
    let docx = with["content"][1]["items"][0]["blocks"][0]["file"]
        .as_str()
        .unwrap();
    assert_eq!(markdown(with), format!("[testing.docx]({docx})"));

    let wopi = &pages("interop/SUT/FileSyncWOPI_OneNoteWithODB.one")[0];
    let md = markdown(wopi);
    assert!(md.starts_with("![]("), "{md}");
    assert!(!md.contains('#'), "an untitled page has no heading: {md}");

    let group = pages("onenote-rs/New Section Group/New Section 1.one");
    assert_eq!(markdown(&group[0]), "# Test Page 2\n\nTest 1\n\nTest 2");

    let media = pages("onenote-rs/New Section Group/New Section 2.one");
    let md = markdown(&media[0]);
    assert!(md.starts_with("# Test Page 3\n\n![]("), "{md}");
    assert!(md.ends_with(".png)"), "{md}");
    let md = markdown(&media[1]);
    assert!(
        md.starts_with("# Test Page 4\n\n[ff-16b-2c-44100hz.mp3]("),
        "{md}"
    );
    assert!(md.ends_with(".mp3)"), "{md}");

    let deleted = pages("onenote-rs/OneNote_RecycleBin/OneNote_DeletedPages.one");
    assert_eq!(markdown(&deleted[0]), "# Te");

    // An untitled page with nothing but its date and blank lines has no markdown field.
    for file in [
        "interop/SUT/InvalidData.one",
        "onenote-rs/non-legacy/New Section 2.one",
        "onenote-rs/non-legacy/New Section 3.one",
    ] {
        for page in pages(file) {
            assert!(page.get("markdown").is_none(), "{file}: {page}");
        }
    }

    // non-legacy's copy of the test page renders as the original does.
    let copy = pages("onenote-rs/non-legacy/New Section 1 2.one");
    let ink = format!("![ink]({})", ink_file(&copy[0]));
    assert_eq!(markdown(&copy[0]), test_page(&ink));
    assert_eq!(markdown(&copy[1]), "# Test Page");
}
