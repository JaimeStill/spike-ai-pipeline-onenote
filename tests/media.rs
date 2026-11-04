//! Media: images and embedded files in the model, and their contents in the --media directory.
//! The expectations are written by hand from what onenote_parser's API gives for each record
//! (its fields, its data's size, and the SHA-256 of the bytes its reader yields), not from the
//! CLI's output.

mod common;

use std::fs::File;

use serde_json::{Value, json};

use common::{NAME, files_under, fixture, run, run_in, scratch, sha256};

/// The committed sections that hold images or embedded files.
const WITH_MEDIA: [&str; 5] = [
    "interop/SUT/OneWithFileData.one",
    "interop/SUT/FileSyncWOPI_OneNoteWithODB.one",
    "onenote-rs/New Section Group/New Section 2.one",
    "onenote-rs/New Section 1.one",
    "onenote-rs/handwriting_recognition.one",
];

/// The SHA-256s of the five distinct contents among them.
const DOCX: &str = "8e99dc44681b8399608f7a181e44d3e41dc36fbacd35a9cfb2d1fc330ac68459";
const CHRYSANTHEMUM: &str = "954f7d96502b5c5fe2e98a5045bca7f5e9ba11e3dbf92a5c0214a6aa4c7f2208";
const DUMMY: &str = "b7702e05282d4dfffe233281443536319d4739946f54ebce194230df8805b650";
const MP3: &str = "d2318cc34b6254cdc2db84b931adad166a4b2b701b4241c27b338b959ac738b0";
const SCREENSHOT: &str = "d6d4898c203cbff35fe92e844bbf404064293314a71d1e6b24e907334e5bdff9";

/// Every image and embedded file in a read's page lines, containers and blocks alike, in order.
fn records(lines: &[Value]) -> Vec<Value> {
    fn walk(value: &Value, found: &mut Vec<Value>) {
        match value {
            Value::Object(fields) => {
                if matches!(
                    fields.get("kind").and_then(Value::as_str),
                    Some("image" | "embedded_file")
                ) {
                    found.push(value.clone());
                }
                fields.values().for_each(|field| walk(field, found));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for line in lines.iter().filter(|line| line["line"] == "page") {
        walk(&line["content"], &mut found);
    }
    found
}

fn read(file: &str) -> Vec<Value> {
    let out = run(&[fixture(file)]);
    assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    assert_eq!(out.stderr, "", "{file}");
    records(&out.lines())
}

/// An embedded .docx, with no type: its file named by its filename's extension.
#[test]
fn an_embedded_document() {
    assert_eq!(
        read("interop/SUT/OneWithFileData.one"),
        [json!({
            "kind": "embedded_file",
            "file": format!("{DOCX}.docx"),
            "filename": "testing.docx",
            "size": 14660,
        })]
    );
}

/// An image with no extension of its own, named by its filename's, and OneNote's text from
/// the picture: 39 lines of what it made of a photograph.
#[test]
fn an_image_with_ocr_text() {
    let mut records = read("interop/SUT/FileSyncWOPI_OneNoteWithODB.one");
    assert_eq!(records.len(), 1);
    let text = records[0]["ocr_text"].as_str().unwrap().to_owned();
    assert!(text.starts_with("尹、气\n歹杯\n火了夕了\n"), "{text:?}");
    assert!(text.ends_with("\n',\nlri"), "{text:?}");
    assert_eq!(text.lines().count(), 39);
    records[0].as_object_mut().unwrap().remove("ocr_text");
    assert_eq!(
        records[0],
        json!({
            "kind": "image",
            "file": format!("{CHRYSANTHEMUM}.jpg"),
            "size": 879394,
            "filename": "Chrysanthemum.jpg",
        })
    );
}

/// A page-level image with its offsets, its own extension, stored as ".png", and its printout
/// page number; then, on the next page, an embedded mp3 of type audio.
#[test]
fn a_page_image_and_an_audio_file() {
    assert_eq!(
        read("onenote-rs/New Section Group/New Section 2.one"),
        [
            json!({
                "kind": "image",
                "offset_horizontal": 0.9166665,
                "offset_vertical": 2.0408335,
                "file": format!("{DUMMY}.png"),
                "size": 27146,
                "extension": "png",
                "filename": "dummy_1.png",
                "printout_page": 1,
            }),
            json!({
                "kind": "embedded_file",
                "file": format!("{MP3}.mp3"),
                "filename": "ff-16b-2c-44100hz.mp3",
                "type": "audio",
                "size": 77279,
            }),
        ]
    );
}

/// The test page's image, with its alt text, in both samples: "New Section 1.one" stores its
/// extension, handwriting_recognition.one doesn't, and stores an empty text, which is left out.
/// Both name the same file.
#[test]
fn the_test_pages_image() {
    let image = json!({
        "kind": "image",
        "file": format!("{SCREENSHOT}.jpg"),
        "size": 90999,
        "filename": "testimages_screenshot.jpg",
        "alt_text": "example images from TESTIMAGES archive",
    });
    let mut with_extension = image.clone();
    with_extension["extension"] = json!("jpg");
    assert_eq!(read("onenote-rs/handwriting_recognition.one"), [image]);
    assert_eq!(read("onenote-rs/New Section 1.one"), [with_extension]);
}

/// With --media, the directory is created, nested, and holds exactly the files the model
/// names, each once, its SHA-256 its name and its size the record's. The lines are the same
/// as without --media.
#[test]
fn media_files_are_named_by_their_sha256() {
    for file in WITH_MEDIA {
        let dir = scratch("media-named").join("made/here");
        let without = run(&[fixture(file)]);
        let out = run(&[
            fixture(file).as_os_str(),
            "--media".as_ref(),
            dir.as_os_str(),
        ]);
        assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
        assert_eq!(out.stdout, without.stdout, "{file}");
        let records = records(&out.lines());
        let mut names: Vec<String> = records
            .iter()
            .map(|r| r["file"].as_str().unwrap().to_owned())
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(files_under(&dir), names, "{file}");
        for record in records {
            let name = record["file"].as_str().unwrap();
            let path = dir.join(name);
            let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
            assert_eq!(sha256(&path), stem, "{file}: {name}");
            let size = std::fs::metadata(&path).unwrap().len();
            assert_eq!(json!(size), record["size"], "{file}: {name}");
        }
    }
}

/// The same content read twice into one directory is one file there.
#[test]
fn a_repeated_content_is_one_file() {
    let dir = scratch("media-repeated");
    for file in [
        "onenote-rs/New Section 1.one",
        "onenote-rs/handwriting_recognition.one",
    ] {
        let out = run(&[
            fixture(file).as_os_str(),
            "--media".as_ref(),
            dir.as_os_str(),
        ]);
        assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
    }
    assert_eq!(files_under(&dir), [format!("{SCREENSHOT}.jpg")]);
}

/// A media directory that can't be created exits 1 with one error line, and the page line
/// that would name its file is never written.
#[test]
fn an_unwritable_media_directory_exits_1() {
    let base = scratch("media-unwritable");
    let blocker = base.join("file");
    File::create(&blocker).unwrap();
    let dir = blocker.join("media");
    let section = fixture("onenote-rs/New Section Group/New Section 2.one");
    let out = run(&[section.as_os_str(), "--media".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert_eq!(
        out.stderr,
        format!(
            "{NAME}: error: media directory {} can't be created: Not a directory (os error 20)\n",
            dir.display()
        )
    );
    let kinds: Vec<Value> = out.lines().iter().map(|l| l["line"].clone()).collect();
    assert_eq!(kinds, [json!("notebook"), json!("section")]);
}

/// Without --media nothing is written to disk: the working directory stays empty, without a
/// file or a directory.
#[test]
fn without_media_nothing_is_written() {
    let dir = scratch("media-none");
    for file in WITH_MEDIA {
        let out = run_in(&dir, &[fixture(file)]);
        assert_eq!(out.code, Some(0), "{file}: {}", out.stderr);
        assert!(!out.stdout.is_empty(), "{file}");
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}
