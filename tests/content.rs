//! Rich text: a page line's content, read from onenote.rs's samples. "New Section 1.one" and
//! the first page of handwriting_recognition.one hold the same test page of headings, runs,
//! note tags, links, lists, tables, an equation and an image. The expectations are written by
//! hand from what onenote_parser's API gives for that page, not from the CLI's output.

mod common;

use serde_json::{Value, json};

use common::{fixture, run};

const SAMPLES: [&str; 2] = [
    "onenote-rs/New Section 1.one",
    "onenote-rs/handwriting_recognition.one",
];

const LOREM: &str = "Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy \
                     eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam \
                     voluptua.";

/// The test page's line in a sample's read.
fn test_page(file: &str) -> Value {
    let out = run(&[fixture(file)]);
    assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    out.lines()
        .into_iter()
        .find(|line| line["line"] == "page")
        .unwrap_or_else(|| panic!("{file}: no page"))
}

/// An element holding one paragraph of the style `style`, its runs `runs`, and nothing more.
fn element(style: &str, runs: Value) -> Value {
    json!({"kind": "element", "blocks": [{"kind": "paragraph", "style": style, "runs": runs}]})
}

/// An element holding one empty paragraph, as OneNote writes a blank line.
fn blank() -> Value {
    json!({"kind": "element", "blocks": [{"kind": "paragraph", "style": "p"}]})
}

/// An element holding one paragraph with the list marker `list` and the text `text`, and the
/// items `children` below it.
fn listed(list: Value, text: &str, children: Value) -> Value {
    let mut item = json!({"kind": "element", "blocks": [{
        "kind": "paragraph", "style": "p", "list": list, "runs": [{"text": text}],
    }]});
    if children.as_array().is_some_and(|c| !c.is_empty()) {
        item["children"] = children;
    }
    item
}

/// A table cell holding one paragraph of `text`.
fn cell(text: &str) -> Value {
    json!({"items": [element("p", json!([{"text": text}]))]})
}

/// The containers come in stored order with their offsets: the title, the main outline, the
/// page's ink, and a last outline. Offsets are half-inch units, as stored.
#[test]
fn containers_come_in_stored_order_with_their_offsets() {
    for file in SAMPLES {
        let content = &test_page(file)["content"];
        let containers = content.as_array().unwrap();
        let at = |i: usize| {
            json!([
                containers[i]["kind"],
                containers[i]["offset_horizontal"],
                containers[i]["offset_vertical"]
            ])
        };
        assert_eq!(at(0), json!(["title", 0.0, 0.0]), "{file}");
        assert_eq!(at(1), json!(["outline", 1.0, 2.4]), "{file}");
        assert_eq!(at(2), json!(["ink", 0.0, 0.0]), "{file}");
        // f32 0.9902231 and 32.098755, as onenote_parser gives them.
        assert_eq!(at(3), json!(["outline", 0.9902231, 32.098755]), "{file}");
        assert_eq!(containers[3]["items"], json!([blank()]), "{file}");
    }
}

/// The title container holds the page's title paragraph and its date and time, each with its
/// stored style.
#[test]
fn the_title_holds_its_styled_paragraphs() {
    for file in SAMPLES {
        let title = &test_page(file)["content"][0];
        assert_eq!(
            title["items"],
            json!([
                element("PageTitle", json!([{"text": "Test Page"}])),
                element(
                    "PageDateTime",
                    json!([{"text": "Tuesday, 27. October 2020"}])
                ),
                element("PageDateTime", json!([{"text": "11:47"}])),
            ]),
            "{file}"
        );
    }
}

/// The main outline's first items: a plain paragraph, a blank line, an h1 heading, and an
/// element whose children are the tagged paragraphs. Each note tag holds its label, shape and
/// status, and a completed one its completion time: onenote_parser's 1288263013 and
/// 1288263010 seconds after 1980-01-01.
#[test]
fn headings_and_note_tags() {
    for file in SAMPLES {
        let items = &test_page(file)["content"][1]["items"];
        assert_eq!(
            items[0],
            element("p", json!([{"text": "ABCDEF"}])),
            "{file}"
        );
        assert_eq!(items[1], blank(), "{file}");
        assert_eq!(
            items[2],
            element("h1", json!([{"text": "ABCDEF"}])),
            "{file}"
        );
        assert_eq!(items[3]["blocks"], blank()["blocks"], "{file}");
        let tagged = |tag: Value, text: &str| {
            json!({"kind": "element", "blocks": [{
                "kind": "paragraph", "style": "p", "tags": [tag], "runs": [{"text": text}],
            }]})
        };
        let children = &items[3]["children"];
        assert_eq!(
            children[0],
            tagged(
                json!({"label": "To Do", "shape": "BlueCheckBox", "status": "open"}),
                "ABCDEF"
            ),
            "{file}"
        );
        assert_eq!(
            children[1],
            tagged(
                json!({"label": "To Do", "shape": "BlueCheckBox", "status": "completed",
                       "completed_time": "2020-10-27T10:50:13Z"}),
                "ABCDEFG"
            ),
            "{file}"
        );
        assert_eq!(
            children[2],
            tagged(
                json!({"label": "Important", "shape": "YellowStar", "status": "completed",
                       "completed_time": "2020-10-27T10:50:10Z"}),
                "ABCDEFGH"
            ),
            "{file}"
        );
        assert_eq!(children[3], blank(), "{file}");
    }
}

/// The formatted paragraph: onenote_parser gives 19 formattings over 18 UTF-16 run indices,
/// one formatting each for bold, italic, underline, strikethrough, subscript, superscript,
/// highlight and font colour, then a hidden HYPERLINK field and the visible link text it
/// targets. The hidden run is left out; its target goes on the link text. Two paragraphs of
/// plain text nest below it, one in the other.
#[test]
fn runs_hold_their_formatting_and_links() {
    for file in SAMPLES {
        let item = &test_page(file)["content"][1]["items"][3]["children"][4];
        assert_eq!(
            item["blocks"][0]["runs"],
            json!([
                {"text": "Lorem", "bold": true},
                {"text": " ipsum "},
                {"text": "dolor", "italic": true},
                {"text": " sit "},
                {"text": "amet", "underline": true},
                {"text": ", consetetur "},
                {"text": "sadipscing", "strikethrough": true},
                {"text": " elitr, "},
                {"text": "sed", "subscript": true},
                {"text": " diam "},
                {"text": "nonumy", "superscript": true},
                {"text": " eirmod tempor "},
                {"text": "invidunt", "highlight": "#ffc000"},
                {"text": " ut "},
                {"text": "labore", "font_color": "#7f7f7f"},
                {"text": " et dolore "},
                {"text": "magna", "link": "https://example.com"},
                {"text": " aliquyam erat, sed diam voluptua."},
            ]),
            "{file}"
        );
        let nested = element("p", json!([{"text": LOREM}]));
        let mut outer = nested.clone();
        outer["children"] = json!([nested]);
        assert_eq!(item["children"], json!([outer]), "{file}");
    }
}

/// A run styled as a link with no HYPERLINK field, as OneNote marks a URL typed as text,
/// targets its own text.
#[test]
fn a_typed_url_links_to_itself() {
    for file in SAMPLES {
        let children = &test_page(file)["content"][1]["items"][3]["children"];
        assert_eq!(children[5], blank(), "{file}");
        assert_eq!(
            children[6],
            element(
                "p",
                json!([{"text": "http://example.com/", "link": "http://example.com/"}])
            ),
            "{file}"
        );
        assert_eq!(children.as_array().unwrap().len(), 7, "{file}");
    }
}

/// Bulleted lists nest three deep, each level with its stored bullet character: "•", "○",
/// and Wingdings' "§". Numbered lists nest decimal, lower-letter and lower-roman, each with
/// its "." suffix; the last of three more decimal items restarts at 1.
#[test]
fn lists_hold_their_markers_and_nest() {
    for file in SAMPLES {
        let items = &test_page(file)["content"][1]["items"];
        assert_eq!(items[4]["blocks"], blank()["blocks"], "{file}");
        let bullet = |b: &str| json!({"bullet": b});
        assert_eq!(
            items[4]["children"],
            json!([listed(
                bullet("•"),
                LOREM,
                json!([listed(
                    bullet("○"),
                    LOREM,
                    json!([listed(bullet("§"), LOREM, json!([]))])
                )])
            )]),
            "{file}"
        );
        assert_eq!(items[5]["blocks"], blank()["blocks"], "{file}");
        let number = |n: &str| json!({"number": n, "suffix": "."});
        assert_eq!(
            items[5]["children"],
            json!([
                listed(
                    number("decimal"),
                    LOREM,
                    json!([listed(
                        number("lower-letter"),
                        LOREM,
                        json!([listed(number("lower-roman"), LOREM, json!([]))])
                    )])
                ),
                listed(number("decimal"), LOREM, json!([])),
                listed(number("decimal"), LOREM, json!([])),
                listed(
                    json!({"number": "decimal", "suffix": ".", "restart": 1}),
                    LOREM,
                    json!([])
                ),
            ]),
            "{file}"
        );
    }
}

/// Two tables: two rows of three cells, A B C over 1 2 3, and one row of two, A B. A cell
/// holds its outline elements.
#[test]
fn tables_hold_rows_of_cells() {
    for file in SAMPLES {
        let items = &test_page(file)["content"][1]["items"];
        let table =
            |rows: Value| json!({"kind": "element", "blocks": [{"kind": "table", "rows": rows}]});
        assert_eq!(
            items[12],
            table(json!([
                [cell("A"), cell("B"), cell("C")],
                [cell("1"), cell("2"), cell("3")],
            ])),
            "{file}"
        );
        assert_eq!(items[15], table(json!([[cell("A"), cell("B")]])), "{file}");
    }
}

/// The rest of the main outline, in order: paragraphs and blank lines, an equation, still a
/// plain italic run until math is read, and an image block, whose fields tests/media.rs
/// checks.
#[test]
fn the_rest_of_the_outline_in_order() {
    for file in SAMPLES {
        let items = test_page(file)["content"][1]["items"].clone();
        let items = items.as_array().unwrap();
        assert_eq!(items.len(), 22, "{file}");
        let lorem = element("p", json!([{"text": LOREM}]));
        let expected = [
            (6, blank()),
            (7, lorem.clone()),
            (8, blank()),
            (9, lorem),
            (10, blank()),
            (11, blank()),
            (13, blank()),
            (14, blank()),
            (16, blank()),
            (17, element("p", json!([{"text": "𝑎=𝑏", "italic": true}]))),
            (18, blank()),
            (20, blank()),
            (21, blank()),
        ];
        for (index, item) in expected {
            assert_eq!(items[index], item, "{file}: item {index}");
        }
        assert_eq!(items[19]["blocks"][0]["kind"], "image", "{file}");
        assert_eq!(items[19]["blocks"].as_array().unwrap().len(), 1, "{file}");
    }
}
