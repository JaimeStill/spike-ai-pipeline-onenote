//! Ink: page drawings and recognised text, read from onenote.rs's samples. Three committed
//! sections hold ink: handwriting_recognition.one, whose first page has 62 ink containers of
//! one stroke each and OneNote's recognition of two lines, and "New Section 1.one" and
//! "non-legacy/New Section 1 2.one", whose test page has one. None holds ink in a paragraph
//! or an outline; src/ink.rs's unit tests cover those drawings. The expectations are written
//! by hand from what onenote_parser's API gives (each stroke's absolute_path(), its
//! container's offsets, and Page::ink_recognition), not from the CLI's output.

mod common;

use std::path::Path;

use serde_json::{Value, json};

use common::{fixture, run, scratch, sha256};

const HANDWRITING: &str = "onenote-rs/handwriting_recognition.one";
const ONE_STROKE: [&str; 2] = [
    "onenote-rs/New Section 1.one",
    "onenote-rs/non-legacy/New Section 1 2.one",
];

/// The box the test page's first stroke spans, in ink units: its absolute path runs from
/// (1363, 38643) to (17926, 40660), at offsets of zero.
fn first_stroke() -> Value {
    json!({"x": 1363.0, "y": 38643.0, "width": 16563.0, "height": 2017.0})
}

fn pages(file: &str) -> Vec<Value> {
    let out = run(&[fixture(file)]);
    assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    out.lines()
        .into_iter()
        .filter(|line| line["line"] == "page")
        .collect()
}

/// A read with --media into `dir`: its page lines.
fn pages_into(file: &str, dir: &Path) -> Vec<Value> {
    let out = run(&[
        fixture(file).as_os_str(),
        "--media".as_ref(),
        dir.as_os_str(),
    ]);
    assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    out.lines()
        .into_iter()
        .filter(|line| line["line"] == "page")
        .collect()
}

fn ink_containers(page: &Value) -> Vec<&Value> {
    page["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|container| container["kind"] == "ink")
        .collect()
}

/// The handwriting page's 62 ink containers are one drawing of 62 strokes. Its bounds span
/// every stroke's points: x from 1363 to 17926, y from 38643 to 54392. Each container keeps
/// its one stroke and the box it spans; the page's second page has no ink.
#[test]
fn a_pages_ink_is_one_drawing() {
    let pages = pages(HANDWRITING);
    assert_eq!(pages.len(), 2);
    let ink = &pages[0]["ink"];
    assert_eq!(ink["strokes"], 62);
    assert_eq!(
        ink["bounds"],
        json!({"x": 1363.0, "y": 38643.0, "width": 16563.0, "height": 15749.0})
    );
    let containers = ink_containers(&pages[0]);
    assert_eq!(containers.len(), 62);
    assert!(
        containers.iter().all(|c| c["strokes"] == 1),
        "{containers:?}"
    );
    assert_eq!(containers[0]["bounds"], first_stroke());
    // A stroke of a recognised "Hello", from (8794, 50272) to (8890, 51690).
    assert!(containers.iter().any(
        |c| c["bounds"] == json!({"x": 8794.0, "y": 50272.0, "width": 96.0, "height": 1418.0})
    ));
    assert_eq!(pages[1].get("ink"), None);
    assert!(ink_containers(&pages[1]).is_empty());
}

/// The test page's one ink container is its drawing, of one stroke.
#[test]
fn one_container_of_one_stroke() {
    for file in ONE_STROKE {
        let pages = pages(file);
        let ink = &pages[0]["ink"];
        assert_eq!(ink["strokes"], 1, "{file}");
        assert_eq!(ink["bounds"], first_stroke(), "{file}");
        let containers = ink_containers(&pages[0]);
        assert_eq!(containers.len(), 1, "{file}");
        assert_eq!(containers[0]["bounds"], first_stroke(), "{file}");
        for page in &pages[1..] {
            assert_eq!(page.get("ink"), None, "{file}");
        }
    }
}

/// OneNote recognised two lines of "Hello World", each word with five guesses in German
/// (LCID 1031): the best is the word's text, the other four its alternatives.
#[test]
fn recognised_words_come_in_lines() {
    let handwriting = pages(HANDWRITING);
    let word = |text: &str, alternatives: [&str; 4]| json!({"text": text, "alternatives": alternatives, "language": 1031});
    assert_eq!(
        handwriting[0]["ink_text"],
        json!([
            [
                word("Hello", ["Hallo", "Hell", "HellO", "Hella"]),
                word("World", ["world", "Worlds", "Word", "World."]),
            ],
            [
                word("Hello", ["Hallo", "Hell", "HellO", "Helle"]),
                word("World", ["world", "Worlds", "Word", "Would"]),
            ],
        ])
    );
    assert_eq!(handwriting[1].get("ink_text"), None);
    for file in ONE_STROKE {
        assert!(
            pages(file)
                .iter()
                .all(|page| page.get("ink_text").is_none())
        );
    }
}

/// With --media, each drawing is an SVG named by its SHA-256 with the extension "svg": an
/// XML document whose root is an SVG element holding one path per stroke, each with its
/// colour and width.
#[test]
fn each_drawing_is_an_svg_named_by_its_sha256() {
    for file in [HANDWRITING, ONE_STROKE[0], ONE_STROKE[1]] {
        let dir = scratch("ink-svg");
        let pages = pages_into(file, &dir);
        let ink = &pages[0]["ink"];
        let name = ink["file"].as_str().unwrap();
        let stem = name.strip_suffix(".svg").expect("an ink file ends .svg");
        let path = dir.join(name);
        assert_eq!(sha256(&path), stem, "{file}");
        let text = std::fs::read_to_string(&path).unwrap();
        let document = roxmltree::Document::parse(&text)
            .unwrap_or_else(|err| panic!("{file}: not well-formed XML: {err}"));
        let root = document.root_element();
        assert_eq!(root.tag_name().name(), "svg", "{file}");
        assert_eq!(
            root.tag_name().namespace(),
            Some("http://www.w3.org/2000/svg")
        );
        let paths: Vec<_> = root.children().filter(|node| node.is_element()).collect();
        assert_eq!(json!(paths.len()), ink["strokes"], "{file}");
        for path in paths {
            assert_eq!(path.tag_name().name(), "path", "{file}");
            assert!(path.attribute("d").unwrap().starts_with("M "), "{file}");
            // No fixture's stroke has a colour, so each is drawn in the default black.
            assert_eq!(path.attribute("stroke"), Some("#000000"), "{file}");
            assert!(path.attribute("stroke-width").is_some(), "{file}");
        }
    }
}

/// The handwriting page's strokes keep their pens' widths as onenote_parser's width() gives
/// them: 13 of 35 ink units, 35 of 18.75, and 14 of 100.
#[test]
fn each_stroke_keeps_its_width() {
    let dir = scratch("ink-width");
    let pages = pages_into(HANDWRITING, &dir);
    let text =
        std::fs::read_to_string(dir.join(pages[0]["ink"]["file"].as_str().unwrap())).unwrap();
    let document = roxmltree::Document::parse(&text).unwrap();
    let widths: Vec<&str> = document
        .descendants()
        .filter_map(|node| node.attribute("stroke-width"))
        .collect();
    assert_eq!(widths.len(), 62);
    let count = |width: &str| widths.iter().filter(|w| **w == width).count();
    assert_eq!((count("35"), count("18.75"), count("100")), (13, 35, 14));
}

/// Two reads write byte-identical SVGs, and the same lines.
#[test]
fn drawings_are_the_same_on_every_read() {
    let first = scratch("ink-first");
    let second = scratch("ink-second");
    let a = pages_into(HANDWRITING, &first);
    let b = pages_into(HANDWRITING, &second);
    assert_eq!(a, b);
    let name = a[0]["ink"]["file"].as_str().unwrap();
    assert_eq!(
        std::fs::read(first.join(name)).unwrap(),
        std::fs::read(second.join(name)).unwrap()
    );
}
