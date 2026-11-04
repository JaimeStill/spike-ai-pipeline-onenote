//! Single sections: each public .one fixture against its hand-written structural expectations
//! in tests/fixtures/sections.json, and the line rules every read keeps.

mod common;

use std::path::Path;

use serde_json::{Value, json};

use common::{error, files_under, fixture, run};

/// The page fields this version writes; a field the expectation leaves out must be absent.
const PAGE_FIELDS: [&str; 6] = [
    "id",
    "level",
    "title",
    "creation_time",
    "modification_time",
    "author",
];

fn expectations() -> Vec<Value> {
    let bytes = std::fs::read(fixture("sections.json")).unwrap();
    serde_json::from_slice::<Value>(&bytes)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

/// The expectations cover every committed .one file of the public sets.
#[test]
fn every_public_section_has_an_expectation() {
    let mut listed: Vec<String> = expectations()
        .iter()
        .map(|e| e["file"].as_str().unwrap().to_owned())
        .collect();
    listed.sort_unstable();
    let mut sections = Vec::new();
    for set in ["interop", "onenote-rs"] {
        for file in files_under(&fixture(set)) {
            if file.ends_with(".one") {
                sections.push(format!("{set}/{file}"));
            }
        }
    }
    sections.sort_unstable();
    assert_eq!(listed, sections);
}

/// Each section reads as its expectation says: its exit code, and either its refusal's reason,
/// or a notebook line, its section line, and a page line per expected page, in order, each
/// with its containers counted by kind.
#[test]
fn every_public_section_reads_as_expected() {
    for expected in expectations() {
        let file = expected["file"].as_str().unwrap();
        let input = fixture(file);
        let out = run(&[&input]);
        assert_eq!(
            out.code,
            expected["code"].as_i64().map(|c| c as i32),
            "{file}"
        );
        if let Some(reason) = expected["reason"].as_str() {
            assert_eq!(out.stdout, "", "{file}");
            assert_eq!(out.stderr, error(&input, reason), "{file}");
            continue;
        }
        assert_eq!(out.stderr, "", "{file}");
        let lines = out.lines();
        let pages = expected["pages"].as_array().unwrap();
        let kinds: Vec<&str> = lines.iter().map(|l| l["line"].as_str().unwrap()).collect();
        let mut want = vec!["notebook", "section"];
        want.extend(pages.iter().map(|_| "page"));
        assert_eq!(kinds, want, "{file}");

        let stem = Path::new(file).file_stem().unwrap().to_str().unwrap();
        assert_eq!(
            lines[0],
            json!({"line": "notebook", "schema": "1", "input": "section", "name": stem}),
            "{file}"
        );
        let section = &expected["section"];
        assert_eq!(
            lines[1],
            json!({
                "line": "section",
                "id": section["id"],
                "path": [section["display_name"]],
                "display_name": section["display_name"],
                "color": section["color"],
            }),
            "{file}"
        );
        for (line, page) in lines[2..].iter().zip(pages) {
            assert_eq!(line["section"], section["id"], "{file}");
            for field in PAGE_FIELDS {
                assert_eq!(line.get(field), page.get(field), "{file}: {field}");
            }
            assert_eq!(kind_counts(line), page["content"], "{file}: content");
        }
    }
}

/// Every line of every read is a JSON object whose `line` field names its kind, with no null,
/// empty string, empty list or empty object anywhere in it but an empty table cell, `{}`, which
/// holds its place in its row; and every time ISO 8601 UTC to the second. Each read's output is byte-identical from one run to the next.
#[test]
fn every_read_keeps_the_line_rules_and_repeats_byte_for_byte() {
    for expected in expectations() {
        let file = expected["file"].as_str().unwrap();
        let first = run(&[fixture(file)]);
        let second = run(&[fixture(file)]);
        assert_eq!(first.stdout, second.stdout, "{file}");
        for line in first.lines() {
            assert!(line["line"].is_string(), "{file}: {line}");
            assert_no_empty(&line, file);
            for field in ["creation_time", "modification_time"] {
                if let Some(time) = line.get(field) {
                    assert!(is_iso8601_second(time.as_str().unwrap()), "{file}: {time}");
                }
            }
        }
    }
}

/// A page line's containers counted by kind, as {"outline": 2, ...}.
fn kind_counts(page: &Value) -> Value {
    let mut counts = serde_json::Map::new();
    for container in page["content"].as_array().into_iter().flatten() {
        let kind = container["kind"].as_str().unwrap().to_owned();
        let count = counts.get(&kind).and_then(Value::as_u64).unwrap_or(0);
        counts.insert(kind, json!(count + 1));
    }
    Value::Object(counts)
}

fn assert_no_empty(value: &Value, file: &str) {
    match value {
        Value::Null => panic!("{file}: a null"),
        Value::String(text) => assert!(!text.is_empty(), "{file}: an empty string"),
        Value::Array(items) => {
            assert!(!items.is_empty(), "{file}: an empty list");
            items.iter().for_each(|item| assert_no_empty(item, file));
        }
        Value::Object(fields) => {
            assert!(!fields.is_empty(), "{file}: an empty object");
            for (name, field) in fields {
                if value["kind"] == "table" && name == "rows" {
                    assert_no_empty_rows(field, file);
                } else {
                    assert_no_empty(field, file);
                }
            }
        }
        Value::Bool(_) | Value::Number(_) => {}
    }
}

/// A table's rows: no row is empty, and a cell may be the empty object, as an empty cell is.
fn assert_no_empty_rows(rows: &Value, file: &str) {
    let rows = rows.as_array().unwrap();
    assert!(!rows.is_empty(), "{file}: a table without rows");
    for row in rows {
        let cells = row.as_array().unwrap();
        assert!(!cells.is_empty(), "{file}: an empty row");
        for cell in cells {
            if cell.as_object().is_none_or(|fields| !fields.is_empty()) {
                assert_no_empty(cell, file);
            }
        }
    }
}

/// Whether `time` has the form 2020-10-27T10:47:23Z.
fn is_iso8601_second(time: &str) -> bool {
    let bytes = time.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            4 | 7 => b == b'-',
            10 => b == b'T',
            13 | 16 => b == b':',
            19 => b == b'Z',
            _ => b.is_ascii_digit(),
        })
}
