//! Notebooks: each committed .onetoc2 against its hand-written structural expectations in
//! tests/fixtures/notebooks.json, each notebook directory read through its .onetoc2, a notebook
//! with a section group, and each package, built here through cab's MSZIP writer, read as its
//! directory is.

mod common;

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};

use common::{NAME, Run, assert_line_rules, files_under, fixture, run, scratch};

fn expectations() -> Vec<Value> {
    let bytes = std::fs::read(fixture("notebooks.json")).unwrap();
    serde_json::from_slice::<Value>(&bytes)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

/// The committed notebook directories: each holds one .onetoc2, so each is a directory input.
const DIRECTORIES: [&str; 6] = [
    "interop/SUT",
    "onenote-rs",
    "onenote-rs/New Section Group",
    "onenote-rs/non-legacy",
    "onenote-rs/object-revision",
    "onenote-rs/OneNote_RecycleBin",
];

/// The expectation of the notebook [`composed`] builds: non-legacy's, whose table of contents
/// also lists a "New Section Group" and a "OneNote_RecycleBin" its directory lacks, with those
/// two added from onenote.rs's samples.
fn composed_expectation() -> Value {
    let group = "2ed96381-1fac-4ca5-bda9-0e3a35478644";
    let mut expected = expectations()
        .into_iter()
        .find(|e| e["file"] == "onenote-rs/non-legacy/Open Notebook.onetoc2")
        .unwrap();
    expected["notebook"]["name"] = json!("composed");
    let entries = expected["entries"].as_array_mut().unwrap();
    entries.extend([
        json!({"line": "section_group", "id": group, "path": ["New Section Group"]}),
        json!({"line": "section", "id": "07e1868e-e7d0-c24e-85c6-7cbe04486764", "parent": group,
               "path": ["New Section Group", "New Section 1"], "color": "#9bbbd2",
               "pages": [{"id": "3fdc5978-8647-8e4c-85de-356300ee58ab", "title": "Test Page 2"}]}),
        json!({"line": "section", "id": "18481a4a-6857-0748-ac96-6327c4ada7c0", "parent": group,
               "path": ["New Section Group", "New Section 2"], "color": "#ab9cb7",
               "pages": [{"id": "e9e2b25d-4462-ea4f-a86c-f167efa63e0d", "title": "Test Page 3"},
                         {"id": "7198bad9-4462-a34e-b5a2-44b2b78e4e60", "title": "Test Page 4"}]}),
    ]);
    expected["warnings"] = json!([
        "/OneNote_RecycleBin: the recycle bin is skipped; its sections and pages are not read"
    ]);
    expected
}

/// The expectations cover every committed .onetoc2 of the public sets.
#[test]
fn every_committed_notebook_has_an_expectation() {
    let mut listed: Vec<String> = expectations()
        .iter()
        .map(|e| e["file"].as_str().unwrap().to_owned())
        .collect();
    listed.sort_unstable();
    let mut tocs = Vec::new();
    for set in ["interop", "onenote-rs"] {
        for file in files_under(&fixture(set)) {
            if file.ends_with(".onetoc2") {
                tocs.push(format!("{set}/{file}"));
            }
        }
    }
    tocs.sort_unstable();
    assert_eq!(listed, tocs);
}

/// Each .onetoc2 reads as its expectation says: its notebook line, its entries depth-first with
/// each section's pages after it, its skipped sections, and its warnings, which name a recycle
/// bin beside it and a section that fails, Interop's Encryption.one, in place of a refusal.
#[test]
fn every_committed_notebook_reads_as_expected() {
    for expected in expectations() {
        let file = expected["file"].as_str().unwrap();
        check(&run(&[fixture(file)]), &expected, "notebook", file);
    }
}

/// A notebook directory reads as the .onetoc2 it holds, the notebook line named for the
/// directory either way.
#[test]
fn a_notebook_directory_reads_as_its_toc() {
    for dir in DIRECTORIES {
        let by_dir = run(&[fixture(dir)]);
        let by_toc = run(&[fixture(dir).join("Open Notebook.onetoc2")]);
        assert_eq!(by_dir.code, Some(0), "{dir}: {}", by_dir.stderr);
        assert_eq!(by_dir.stdout, by_toc.stdout, "{dir}");
        assert_eq!(by_dir.stderr, by_toc.stderr, "{dir}");
    }
}

/// A section group's line comes in its table of contents' place, its sections after it with
/// the group as their parent and its name in their paths; a recycle bin the table of contents
/// lists is skipped with the warning, and its pages are never written.
#[test]
fn a_section_group_holds_its_sections() {
    let out = run(&[composed()]);
    check(&out, &composed_expectation(), "notebook", "composed");
    assert!(!out.stdout.contains("OneNote_DeletedPages"));
}

/// Each package, built from a notebook directory's OneNote files, reads as the directory does
/// in every line but the notebook line's input kind, its warnings included.
#[test]
fn a_package_reads_as_its_directory() {
    let packages = scratch("packages");
    let mut dirs: Vec<PathBuf> = DIRECTORIES.iter().map(|dir| fixture(dir)).collect();
    dirs.push(composed());
    for dir in dirs {
        let package = package(&dir, &packages);
        let by_dir = run(&[&dir]);
        let by_package = run(&[&package]);
        let name = dir.display();
        assert_eq!(by_package.code, Some(0), "{name}: {}", by_package.stderr);
        assert_eq!(by_package.stderr, by_dir.stderr, "{name}");
        let (mut lines, want) = (by_package.lines(), by_dir.lines());
        assert_eq!(lines[0]["input"], "package", "{name}");
        lines[0]["input"] = json!("notebook");
        assert_eq!(lines, want, "{name}");
    }
    let recycled = run(&[packages.join("onenote-rs.onepkg")]);
    assert!(
        recycled
            .stderr
            .contains("/OneNote_RecycleBin: the recycle bin is skipped")
    );
}

/// Every line of every notebook read keeps the line rules, and each read's output is
/// byte-identical from one run to the next.
#[test]
fn every_notebook_read_keeps_the_line_rules_and_repeats_byte_for_byte() {
    let mut inputs: Vec<PathBuf> = expectations()
        .iter()
        .map(|e| fixture(e["file"].as_str().unwrap()))
        .collect();
    inputs.push(composed());
    for input in inputs {
        let name = input.display().to_string();
        let first = run(&[&input]);
        let second = run(&[&input]);
        assert_eq!(first.stdout, second.stdout, "{name}");
        for line in first.lines() {
            assert_line_rules(&line, &name);
        }
    }
}

/// Checks a read against its expectation: exit code, warnings, the notebook line in full, then
/// each line in order, a page line by its id, section and title.
fn check(out: &Run, expected: &Value, input: &str, file: &str) {
    assert_eq!(
        out.code,
        expected["code"].as_i64().map(|c| c as i32),
        "{file}"
    );
    let warnings: String = expected["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| format!("{NAME}: warning: {}\n", w.as_str().unwrap()))
        .collect();
    assert_eq!(out.stderr, warnings, "{file}");

    let mut notebook = json!({"line": "notebook", "schema": "1", "input": input});
    for (field, value) in expected["notebook"].as_object().unwrap() {
        notebook[field] = value.clone();
    }
    let mut want = vec![notebook];
    for entry in expected["entries"].as_array().unwrap() {
        let mut line = entry.clone();
        let fields = line.as_object_mut().unwrap();
        let pages = fields.remove("pages");
        if entry["line"] != "skipped" {
            let name = entry["path"].as_array().unwrap().last().unwrap().clone();
            fields.insert("display_name".to_owned(), name);
        }
        want.push(line);
        for page in pages
            .as_ref()
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut line = json!({"line": "page", "id": page["id"], "section": entry["id"]});
            if let Some(title) = page.get("title") {
                line["title"] = title.clone();
            }
            want.push(line);
        }
    }
    let lines: Vec<Value> = out.lines().into_iter().map(projected).collect();
    assert_eq!(lines, want, "{file}");
}

/// A page line cut to the fields an expectation gives; other lines whole.
fn projected(line: Value) -> Value {
    if line["line"] != "page" {
        return line;
    }
    let mut page = json!({"line": "page", "id": line["id"], "section": line["section"]});
    if let Some(title) = line.get("title") {
        page["title"] = title.clone();
    }
    page
}

/// non-legacy's notebook with onenote.rs's "New Section Group" and "OneNote_RecycleBin" added,
/// as its table of contents lists them, built once under Cargo's temporary directory.
fn composed() -> PathBuf {
    static COMPOSED: OnceLock<PathBuf> = OnceLock::new();
    COMPOSED
        .get_or_init(|| {
            let dir = scratch("composed");
            for (from, into) in [
                ("onenote-rs/non-legacy", ""),
                ("onenote-rs/New Section Group", "New Section Group"),
                ("onenote-rs/OneNote_RecycleBin", "OneNote_RecycleBin"),
            ] {
                std::fs::create_dir_all(dir.join(into)).unwrap();
                for file in files_under(&fixture(from)) {
                    std::fs::copy(fixture(from).join(&file), dir.join(into).join(&file)).unwrap();
                }
            }
            dir
        })
        .clone()
}

/// A .onepkg of `dir`'s OneNote files, an MSZIP cabinet named for the directory and written
/// into `into`, its entries named with Windows separators as an export names them.
fn package(dir: &Path, into: &Path) -> PathBuf {
    let files: Vec<String> = files_under(dir)
        .into_iter()
        .filter(|file| file.ends_with(".one") || file.ends_with(".onetoc2"))
        .collect();
    let mut builder = cab::CabinetBuilder::new();
    let folder = builder.add_folder(cab::CompressionType::MsZip);
    for file in &files {
        folder.add_file(file.replace('/', "\\"));
    }
    let mut writer = builder.build(Cursor::new(Vec::new())).unwrap();
    while let Some(mut entry) = writer.next_file().unwrap() {
        let file = entry.file_name().replace('\\', "/");
        entry
            .write_all(&std::fs::read(dir.join(file)).unwrap())
            .unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    let name = dir.file_name().unwrap().to_str().unwrap();
    let path = into.join(format!("{name}.onepkg"));
    std::fs::write(&path, bytes).unwrap();
    path
}
