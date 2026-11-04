//! What the tests share: running the built binary, and the paths of fixtures and test-built
//! inputs.

#![allow(
    dead_code,
    reason = "each test crate compiles this module and uses only part of it"
)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_spike-ai-pipeline-onenote");

/// The prefix of every error and warning line.
pub const NAME: &str = "spike-ai-pipeline-onenote";

/// One run of the binary: its exit code, stdout and stderr.
pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// stdout's lines, each parsed as JSON.
    pub fn lines(&self) -> Vec<Value> {
        self.stdout
            .lines()
            .map(|line| serde_json::from_str(line).expect("each stdout line is JSON"))
            .collect()
    }
}

pub fn run<S: AsRef<OsStr>>(args: &[S]) -> Run {
    finish(Command::new(BIN).args(args))
}

/// One run of the binary in the working directory `dir`.
pub fn run_in<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Run {
    finish(Command::new(BIN).args(args).current_dir(dir))
}

/// One run of the binary with its stdout on `stdout`.
pub fn run_to<S: AsRef<OsStr>>(stdout: impl Into<Stdio>, args: &[S]) -> Run {
    finish(Command::new(BIN).args(args).stdout(stdout))
}

fn finish(command: &mut Command) -> Run {
    let out = command.output().unwrap();
    Run {
        code: out.status.code(),
        stdout: String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    }
}

/// A committed fixture, by its path under tests/fixtures.
pub fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

/// A test-built input, written under Cargo's temporary directory for integration tests.
pub fn built(name: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// A committed fixture with one byte set to `value`, written as a test-built input.
pub fn patched(path: &str, offset: usize, value: u8, name: &str) -> PathBuf {
    let mut bytes = std::fs::read(fixture(path)).unwrap();
    bytes[offset] = value;
    built(name, &bytes)
}

/// An empty directory under Cargo's temporary directory for integration tests, emptied if a
/// previous run left it.
pub fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// The error line a refusal of `input` for `reason` writes.
pub fn error(input: &Path, reason: &str) -> String {
    format!("{NAME}: error: {}: {reason}\n", input.display())
}

/// A file's lowercase hex SHA-256, by sha256sum, so the tests take no hashing crate.
pub fn sha256(path: &Path) -> String {
    let out = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(out.status.success(), "sha256sum {}", path.display());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

/// Every file under `dir`, as paths relative to it, sorted.
pub fn files_under(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap();
                found.push(relative.to_str().unwrap().to_owned());
            }
        }
    }
    found.sort_unstable();
    found
}

/// The rules every line keeps: a JSON object whose `line` field names its kind, with no null,
/// empty string, empty list or empty object anywhere in it but an empty table cell, `{}`, which
/// holds its place in its row; and every time ISO 8601 UTC to the second. `file` names the
/// input in failures.
pub fn assert_line_rules(line: &Value, file: &str) {
    assert!(line["line"].is_string(), "{file}: {line}");
    assert_no_empty(line, file);
    for field in ["creation_time", "modification_time"] {
        if let Some(time) = line.get(field) {
            assert!(is_iso8601_second(time.as_str().unwrap()), "{file}: {time}");
        }
    }
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
