//! The public fixture sets: each pinned to its upstream commit, listed by SHA-256 in its
//! sources.json, with its LICENSE beside it.

mod common;

use serde_json::Value;

use common::{files_under, fixture, sha256};

/// Each set: its directory under tests/fixtures, its repository, its pinned commit, and its
/// license.
const SETS: [(&str, &str, &str, &str); 2] = [
    (
        "interop",
        "https://github.com/OfficeDev/Interop-TestSuites",
        "fe87ed3253de01804a2ae6e1d0015943da6023f8",
        "MIT; LICENSE",
    ),
    (
        "onenote-rs",
        "https://github.com/msiemens/onenote.rs",
        "727389a674665dc07e0c31818cd4b54b64b9c684",
        "MPL-2.0; LICENSE",
    ),
];

/// Every file of a set but its sources.json has one sources entry, at the set's pin and under
/// its license, whose SHA-256 matches it; its LICENSE is among them.
#[test]
fn every_fixture_file_has_a_matching_sources_entry() {
    for (set, repository, commit, license) in SETS {
        let dir = fixture(set);
        let sources: Value =
            serde_json::from_slice(&std::fs::read(dir.join("sources.json")).unwrap()).unwrap();
        let entries = sources.as_array().unwrap();
        let listed: Vec<&str> = entries
            .iter()
            .map(|e| e["file"].as_str().unwrap())
            .collect();
        let mut files = files_under(&dir);
        files.retain(|file| file != "sources.json");
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, files, "{set}");
        assert!(listed.contains(&"LICENSE"), "{set}");
        for entry in entries {
            let file = entry["file"].as_str().unwrap();
            assert_eq!(entry["repository"], repository, "{set}/{file}");
            assert_eq!(entry["commit"], commit, "{set}/{file}");
            assert_eq!(entry["license"], license, "{set}/{file}");
            assert!(entry["path"].is_string(), "{set}/{file}");
            assert_eq!(
                Some(sha256(&dir.join(file)).as_str()),
                entry["sha256"].as_str(),
                "{set}/{file}"
            );
        }
    }
}

/// onenote.rs's set holds its maintainer's samples alone: never Joplin's AGPL samples, nor the
/// Interop file it vendors, which the Interop set holds.
#[test]
fn the_onenote_rs_set_holds_no_third_party_sample() {
    for file in files_under(&fixture("onenote-rs")) {
        assert!(!file.starts_with("joplin/"), "{file}");
        assert!(!file.starts_with("onenote-2016/"), "{file}");
    }
}
