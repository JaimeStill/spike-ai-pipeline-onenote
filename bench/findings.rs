//! The findings run of evidence 17 for the CLI alone: the fields it recovers, per field kind,
//! against the public fixtures' hand-written expectations, and its time and peak memory on every
//! committed input and on three large sections fetched for the run. scripts/findings.sh builds
//! it and runs it (`mise run findings`); the check only compiles it. It prints the tables and
//! writes one results file, where the script says.
//!
//! The expectations it scores against, each written by hand from what onenote_parser's API
//! gives, not from the CLI's output:
//! - tests/fixtures/sections.json: each committed .one's exit code or refusal, its notebook and
//!   section lines, and each page's fields and containers counted by kind;
//! - tests/fixtures/notebooks.json: each committed .onetoc2's exit code, notebook line, entries
//!   in order with their pages, skipped sections, and warnings;
//! - tests/fixtures/content.json: content fields, each a JSON pointer into one page line.
//!
//! tests/sections.rs, tests/notebooks.rs and tests/content.rs assert the same files, so a run
//! of a binary that passes the check scores every field recovered; the run records it of the
//! release binary itself. There is no authored notebook and so no authored truth (the goal
//! record's escalation of 2026-11-04).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: findings --bin <cli> --rusage <rusage> --large <dir> --state <dir> \
--out <results.json> --commit <rev> [--runs <n>]";

/// The prefix of the CLI's error and warning lines.
const NAME: &str = "spike-ai-pipeline-onenote";

/// The large sections the run fetches, never committed: the name each has in --large, its
/// source, and its SHA-256, which scripts/findings.sh checks before the run.
const LARGE: [(&str, &str, &str, &str, &str, &str); 3] = [
    (
        "LargeData.one",
        "https://github.com/OfficeDev/Interop-TestSuites",
        "fe87ed3253de01804a2ae6e1d0015943da6023f8",
        "FileSyncandWOPI/Setup/SUT/LargeData.one",
        "MIT",
        "034d91a640d655a522efda1d98de4ec4c952aaa02e5ad59f7c35537ccf9c2ba1",
    ),
    (
        "Large Desktop.one",
        "https://github.com/msiemens/onenote.rs",
        "727389a674665dc07e0c31818cd4b54b64b9c684",
        "crates/parser/tests/samples/Large Desktop.one",
        "MPL-2.0",
        "0a568f02de8bdc4196dfad97ce2ecac04456efd9376c275ff67af412a4604668",
    ),
    (
        "Large OneDrive.one",
        "https://github.com/msiemens/onenote.rs",
        "727389a674665dc07e0c31818cd4b54b64b9c684",
        "crates/parser/tests/samples/Large OneDrive.one",
        "MPL-2.0",
        "713980fd942180f64910b71fc6236cd22f407facc9b8bd25e99b2eeb1ac9651a",
    ),
];

/// The arguments scripts/findings.sh passes.
struct Args {
    /// The release binary: static musl, as released.
    bin: PathBuf,
    rusage: PathBuf,
    /// The directory holding the LARGE sections.
    large: PathBuf,
    state: PathBuf,
    out: PathBuf,
    commit: String,
    runs: usize,
}

fn args() -> Result<Args, String> {
    let mut map = BTreeMap::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(flag) = args.next() {
        let flag = flag.into_string().map_err(|_| USAGE.to_owned())?;
        let value = args.next().ok_or_else(|| USAGE.to_owned())?;
        map.insert(flag, value);
    }
    let runs = match map.remove("--runs") {
        Some(runs) => runs
            .to_str()
            .and_then(|runs| runs.parse().ok())
            .filter(|runs| *runs > 0)
            .ok_or_else(|| USAGE.to_owned())?,
        None => 3,
    };
    let mut take = |flag: &str| map.remove(flag).ok_or_else(|| USAGE.to_owned());
    let args = Args {
        bin: take("--bin")?.into(),
        rusage: take("--rusage")?.into(),
        large: take("--large")?.into(),
        state: take("--state")?.into(),
        out: take("--out")?.into(),
        commit: take("--commit")?
            .into_string()
            .map_err(|_| USAGE.to_owned())?,
        runs,
    };
    if !map.is_empty() {
        return Err(USAGE.to_owned());
    }
    Ok(args)
}

fn main() {
    let args = match args() {
        Ok(args) => args,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    if let Err(err) = run(&args) {
        eprintln!("findings: {err}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), String> {
    let started = now();
    // Relative to the repository root, where scripts/findings.sh runs this, so the paths the
    // CLI writes into its errors, and the results file keeps, name no home directory.
    let fixtures = Path::new("tests/fixtures");

    let sections = read_json(&fixtures.join("sections.json"))?;
    let notebooks = read_json(&fixtures.join("notebooks.json"))?;
    let recovery = json!({
        "sections": score_sections(&args.bin, fixtures, &sections)?.into_json(),
        "notebooks": score_notebooks(&args.bin, fixtures, &notebooks)?.into_json(),
        "content": score_content(&args.bin, fixtures)?.into_json(),
    });

    let floor = measure(args, "floor", Path::new("/bin/true"), &[])?;
    // Every committed input, each with the exit code its expectation gives, then the large
    // sections, which must read.
    let mut inputs: Vec<(String, PathBuf, i64)> = Vec::new();
    for expected in sections
        .as_array()
        .into_iter()
        .chain(notebooks.as_array())
        .flatten()
    {
        let file = expected["file"]
            .as_str()
            .ok_or("an expectation without a file")?;
        let code = expected["code"]
            .as_i64()
            .ok_or("an expectation without a code")?;
        inputs.push((format!("tests/fixtures/{file}"), fixtures.join(file), code));
    }
    for (name, ..) in LARGE {
        inputs.push((name.to_owned(), args.large.join(name), 0));
    }
    let mut timing = Vec::new();
    for (name, path, code) in &inputs {
        for media in [false, true] {
            eprintln!(
                "findings: timing {name}{}",
                if media { " --media" } else { "" }
            );
            timing.push(time_input(args, name, path, *code, media)?);
        }
    }

    let mut large = Vec::new();
    for (name, repository, commit, path, license, _) in LARGE {
        let file = args.large.join(name);
        large.push(json!({
            "name": name,
            "repository": repository,
            "commit": commit,
            "path": path,
            "license": license,
            "sha256": sha256(&file)?,
            "size": size(&file)?,
        }));
    }

    let results = json!({
        "schema": 1,
        "started": started,
        "finished": now(),
        "cli": cli(args)?,
        "machine": machine(),
        "recovery": recovery,
        "rusage_floor": floor,
        "large": large,
        "timing": timing,
    });
    print_summary(&results);
    if let Some(dir) = args.out.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&results).map_err(|e| e.to_string())? + "\n";
    fs::write(&args.out, text).map_err(|e| format!("{}: {e}", args.out.display()))?;
    eprintln!("findings: wrote {}", args.out.display());
    Ok(())
}

// ---- Scoring ------------------------------------------------------------------------------

/// The counts of one field kind.
#[derive(Default)]
struct Tally {
    expected: u64,
    recovered: u64,
    mismatched: u64,
    missing: u64,
    /// Fields an expectation says are absent, and how many were.
    absent_expected: u64,
    absent_ok: u64,
}

/// The scores of one group of files against one expectations file.
struct Score {
    files: Vec<String>,
    kinds: BTreeMap<String, Tally>,
    /// Kinds in the order they were first scored, for the tables.
    order: Vec<String>,
    misses: Vec<Value>,
}

impl Score {
    fn new() -> Self {
        Score {
            files: Vec::new(),
            kinds: BTreeMap::new(),
            order: Vec::new(),
            misses: Vec::new(),
        }
    }

    fn tally(&mut self, kind: &str) -> &mut Tally {
        if !self.kinds.contains_key(kind) {
            self.order.push(kind.to_owned());
        }
        self.kinds.entry(kind.to_owned()).or_default()
    }

    /// Records one expected value: `got` against `want`. `at` names the record in the file.
    fn score(&mut self, file: &str, at: &str, kind: &str, want: &Value, got: Option<&Value>) {
        let tally = self.tally(kind);
        tally.expected += 1;
        let outcome = match got {
            Some(got) if got == want => {
                tally.recovered += 1;
                return;
            }
            Some(_) => {
                tally.mismatched += 1;
                "mismatched"
            }
            None => {
                tally.missing += 1;
                "missing"
            }
        };
        self.misses.push(json!({
            "file": file, "at": at, "kind": kind, "outcome": outcome,
            "expected": clip(want), "got": got.map(clip),
        }));
    }

    /// Records a field an expectation says is absent.
    fn absent(&mut self, file: &str, at: &str, kind: &str, got: Option<&Value>) {
        let tally = self.tally(kind);
        tally.absent_expected += 1;
        match got {
            None => tally.absent_ok += 1,
            Some(got) => self.misses.push(json!({
                "file": file, "at": at, "kind": kind, "outcome": "unexpected",
                "expected": null, "got": clip(got),
            })),
        }
    }

    /// Records `want` when it is given, else that `got` is absent.
    fn either(
        &mut self,
        file: &str,
        at: &str,
        kind: &str,
        want: Option<&Value>,
        got: Option<&Value>,
    ) {
        match want {
            Some(want) => self.score(file, at, kind, want, got),
            None => self.absent(file, at, kind, got),
        }
    }

    fn into_json(self) -> Value {
        let kinds: Vec<Value> = self
            .order
            .iter()
            .map(|kind| {
                let t = &self.kinds[kind];
                let mut row = json!({
                    "kind": kind,
                    "expected": t.expected,
                    "recovered": t.recovered,
                    "mismatched": t.mismatched,
                    "missing": t.missing,
                });
                if t.absent_expected > 0 {
                    row["absent_expected"] = json!(t.absent_expected);
                    row["absent_ok"] = json!(t.absent_ok);
                }
                row
            })
            .collect();
        json!({"files": self.files, "kinds": kinds, "misses": self.misses})
    }
}

/// A value shortened for the misses list: a long string keeps its first 160 characters.
fn clip(value: &Value) -> Value {
    match value.as_str() {
        Some(text) if text.chars().count() > 160 => {
            json!(format!("{}...", text.chars().take(160).collect::<String>()))
        }
        _ => value.clone(),
    }
}

/// One read of an input by the CLI: its exit code, stdout's lines, and stderr's lines.
struct Read {
    code: Option<i32>,
    lines: Vec<Value>,
    stderr: Vec<String>,
}

fn read(bin: &Path, input: &Path) -> Result<Read, String> {
    let out = Command::new(bin)
        .arg(input)
        .output()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    let lines = String::from_utf8(out.stdout)
        .map_err(|e| format!("{}: {e}", input.display()))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{}: {e}", input.display())))
        .collect::<Result<_, _>>()?;
    let stderr = String::from_utf8_lossy(&out.stderr)
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(Read {
        code: out.status.code(),
        lines,
        stderr,
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn items(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// The kinds of a read's lines, in order.
fn line_kinds(lines: &[Value]) -> Value {
    json!(lines.iter().map(|l| l["line"].clone()).collect::<Vec<_>>())
}

/// The page fields sections.json gives; one it leaves out must be absent.
const PAGE_FIELDS: [(&str, &str); 5] = [
    ("level", "Page level"),
    ("title", "Page title"),
    ("creation_time", "Page created"),
    ("modification_time", "Page modified"),
    ("author", "Page author"),
];

/// Each committed .one against sections.json: its exit code, and either its refusal's reason
/// or its line kinds, its notebook and section lines field by field, and each page in order,
/// field by field, with its containers counted by kind.
fn score_sections(bin: &Path, fixtures: &Path, expectations: &Value) -> Result<Score, String> {
    let mut score = Score::new();
    for expected in items(expectations) {
        let file = expected["file"]
            .as_str()
            .ok_or("an expectation without a file")?;
        score.files.push(file.to_owned());
        let input = fixtures.join(file);
        let out = read(bin, &input)?;
        score.score(
            file,
            "",
            "Exit code",
            &expected["code"],
            out.code.map(Value::from).as_ref(),
        );
        if let Some(reason) = expected.get("reason") {
            let prefix = format!("{NAME}: error: {}: ", input.display());
            let got = match out.stderr.as_slice() {
                [line] => line.strip_prefix(&prefix).map(Value::from),
                _ => None,
            };
            score.score(file, "", "Refusal reason", reason, got.as_ref());
            score.score(
                file,
                "",
                "Line kinds",
                &json!([]),
                Some(&line_kinds(&out.lines)),
            );
            continue;
        }
        let pages = items(&expected["pages"]);
        let mut kinds = vec![json!("notebook"), json!("section")];
        kinds.extend(pages.iter().map(|_| json!("page")));
        score.score(
            file,
            "",
            "Line kinds",
            &json!(kinds),
            Some(&line_kinds(&out.lines)),
        );
        let warnings = (!out.stderr.is_empty()).then(|| json!(out.stderr));
        score.absent(file, "", "Warning", warnings.as_ref());

        let stem = Path::new(file)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        let notebook = json!({"line": "notebook", "schema": "1", "input": "section", "name": stem});
        score.score(
            file,
            "notebook",
            "Notebook line",
            &notebook,
            out.lines.first(),
        );

        let section = &expected["section"];
        let got = out.lines.iter().find(|l| l["line"] == "section");
        let field = |name: &str| got.and_then(|l| l.get(name));
        score.score(file, "section", "Section id", &section["id"], field("id"));
        let name = &section["display_name"];
        score.score(
            file,
            "section",
            "Section display name",
            name,
            field("display_name"),
        );
        score.score(
            file,
            "section",
            "Section path",
            &json!([name]),
            field("path"),
        );
        score.either(
            file,
            "section",
            "Section colour",
            section.get("color"),
            field("color"),
        );
        score.absent(file, "section", "Section parent", field("parent"));

        let got_pages: Vec<&Value> = out.lines.iter().filter(|l| l["line"] == "page").collect();
        for (i, page) in pages.iter().enumerate() {
            let at = format!("page {i}");
            let got = got_pages.get(i).copied();
            let field = |name: &str| got.and_then(|l| l.get(name));
            score.score(file, &at, "Page id", &page["id"], field("id"));
            score.score(file, &at, "Page section", &section["id"], field("section"));
            for (name, kind) in PAGE_FIELDS {
                score.either(file, &at, kind, page.get(name), field(name));
            }
            let counts = got.map(container_counts).unwrap_or_default();
            let want = page["content"].as_object().cloned().unwrap_or_default();
            let mut kinds: Vec<&String> = want.keys().chain(counts.keys()).collect();
            kinds.sort_unstable();
            kinds.dedup();
            for kind in kinds {
                let label = format!("Containers: {kind}");
                let got = counts.get(kind).map(|n| json!(n));
                score.either(file, &at, &label, want.get(kind), got.as_ref());
            }
        }
    }
    Ok(score)
}

/// A page line's containers counted by kind.
fn container_counts(page: &Value) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for container in items(&page["content"]) {
        let kind = container["kind"].as_str().unwrap_or("?").to_owned();
        *counts.entry(kind).or_default() += 1;
    }
    counts
}

/// Each committed .onetoc2 against notebooks.json: its exit code, its warnings, its notebook
/// line field by field, its lines' kinds and entries in order, each section's fields and its
/// pages by id and title, and each skipped section.
fn score_notebooks(bin: &Path, fixtures: &Path, expectations: &Value) -> Result<Score, String> {
    let mut score = Score::new();
    for expected in items(expectations) {
        let file = expected["file"]
            .as_str()
            .ok_or("an expectation without a file")?;
        score.files.push(file.to_owned());
        let out = read(bin, &fixtures.join(file))?;
        score.score(
            file,
            "",
            "Exit code",
            &expected["code"],
            out.code.map(Value::from).as_ref(),
        );

        let mut extra = out.stderr.clone();
        for warning in items(&expected["warnings"]) {
            let line = format!("{NAME}: warning: {}", warning.as_str().unwrap_or_default());
            let found = extra.iter().position(|l| *l == line);
            let got = found.map(|i| Value::from(extra.remove(i)));
            score.score(file, "", "Warning", &json!(line), got.as_ref());
        }
        let extra = (!extra.is_empty()).then(|| json!(extra));
        score.absent(file, "", "Warning", extra.as_ref());

        let notebook = &expected["notebook"];
        let got = out.lines.first().filter(|l| l["line"] == "notebook");
        let field = |name: &str| got.and_then(|l| l.get(name));
        score.score(
            file,
            "notebook",
            "Notebook schema",
            &json!("1"),
            field("schema"),
        );
        score.score(
            file,
            "notebook",
            "Notebook input kind",
            &json!("notebook"),
            field("input"),
        );
        score.score(
            file,
            "notebook",
            "Notebook name",
            &notebook["name"],
            field("name"),
        );
        score.score(
            file,
            "notebook",
            "Notebook id",
            &notebook["id"],
            field("id"),
        );
        score.either(
            file,
            "notebook",
            "Notebook colour",
            notebook.get("color"),
            field("color"),
        );

        let entries = items(&expected["entries"]);
        let mut kinds = vec![json!("notebook")];
        for entry in entries {
            kinds.push(entry["line"].clone());
            kinds.extend(items(&entry["pages"]).iter().map(|_| json!("page")));
        }
        score.score(
            file,
            "",
            "Line kinds",
            &json!(kinds),
            Some(&line_kinds(&out.lines)),
        );

        for entry in entries {
            if entry["line"] == "skipped" {
                let at = format!("skipped {}", entry["name"]);
                let got = out
                    .lines
                    .iter()
                    .find(|l| l["line"] == "skipped" && l["name"] == entry["name"]);
                score.score(file, &at, "Skipped section", entry, got);
                continue;
            }
            let at = format!("section {}", entry["id"]);
            let index = out
                .lines
                .iter()
                .position(|l| l["line"] == entry["line"] && l["id"] == entry["id"]);
            let got = index.map(|i| &out.lines[i]);
            let id = got.map(|l| &l["id"]);
            score.score(file, &at, "Section id", &entry["id"], id);
            let field = |name: &str| got.and_then(|l| l.get(name));
            score.score(file, &at, "Section path", &entry["path"], field("path"));
            let name = items(&entry["path"]).last().cloned().unwrap_or_default();
            score.score(
                file,
                &at,
                "Section display name",
                &name,
                field("display_name"),
            );
            score.either(
                file,
                &at,
                "Section colour",
                entry.get("color"),
                field("color"),
            );
            score.either(
                file,
                &at,
                "Section parent",
                entry.get("parent"),
                field("parent"),
            );
            // The section's page lines follow its line.
            let got_pages: Vec<&Value> = index
                .map(|i| {
                    out.lines[i + 1..]
                        .iter()
                        .take_while(|l| l["line"] == "page")
                        .collect()
                })
                .unwrap_or_default();
            for (i, page) in items(&entry["pages"]).iter().enumerate() {
                let at = format!("{at} page {i}");
                let got = got_pages.get(i).copied();
                let field = |name: &str| got.and_then(|l| l.get(name));
                score.score(file, &at, "Page id", &page["id"], field("id"));
                score.score(file, &at, "Page section", &entry["id"], field("section"));
                score.either(file, &at, "Page title", page.get("title"), field("title"));
            }
        }
    }
    Ok(score)
}

/// The content fields of content.json, each by its JSON pointer into its page's line in each
/// of its files' reads; a null is a field that must be absent.
fn score_content(bin: &Path, fixtures: &Path) -> Result<Score, String> {
    let expectations = read_json(&fixtures.join("content.json"))?;
    let mut score = Score::new();
    for entry in items(&expectations["pages"]) {
        for file in items(&entry["files"]) {
            let file = file
                .as_str()
                .ok_or("a content entry's file isn't a string")?;
            if !score.files.iter().any(|f| f == file) {
                score.files.push(file.to_owned());
            }
            let out = read(bin, &fixtures.join(file))?;
            let page = out
                .lines
                .iter()
                .find(|l| l["line"] == "page" && l["id"] == entry["page"]);
            for field in items(&entry["fields"]) {
                let kind = field["kind"].as_str().unwrap_or("?");
                let pointer = field["pointer"].as_str().unwrap_or_default();
                let at = format!("page {} {pointer}", entry["page"]);
                let got = page.and_then(|p| p.pointer(pointer));
                let want = Some(&field["value"]).filter(|v| !v.is_null());
                score.either(file, &at, kind, want, got);
            }
        }
    }
    Ok(score)
}

// ---- Timing -------------------------------------------------------------------------------

/// One run through the rusage wrapper, its stdout and stderr into the state's out/.
fn measure(
    args: &Args,
    label: &str,
    program: &Path,
    program_args: &[&OsStr],
) -> Result<Value, String> {
    let dir = args.state.join("out");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let result = Command::new(&args.rusage)
        .arg("3600")
        .arg(dir.join(format!("{label}.jsonl")))
        .arg(dir.join(format!("{label}.stderr")))
        .arg(program)
        .args(program_args)
        .output()
        .map_err(|e| format!("{}: {e}", args.rusage.display()))?;
    if !result.status.success() {
        return Err(format!(
            "rusage failed on {label}: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    let line = String::from_utf8_lossy(&result.stdout);
    let mut fields = Map::new();
    for pair in line.split_whitespace() {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let value = value.parse::<i64>().map_or(json!(value), |n| json!(n));
        fields.insert(key.to_owned(), value);
    }
    Ok(Value::Object(fields))
}

/// An input's runs, with or without --media: each run's time and peak memory, their medians,
/// and what the last run read and wrote. Each run must exit with `code`.
fn time_input(
    args: &Args,
    name: &str,
    path: &Path,
    code: i64,
    media: bool,
) -> Result<Value, String> {
    let slug: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = format!("{slug}{}", if media { "-media" } else { "" });
    let dir = args.state.join("media").join(&slug);
    let mut measured = Vec::new();
    let mut outputs = Vec::new();
    let mut written = json!(null);
    let stdout = args.state.join("out").join(format!("{slug}.jsonl"));
    let stderr = args.state.join("out").join(format!("{slug}.stderr"));
    for _ in 0..args.runs {
        // Each run writes into an empty directory, so every content is written.
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut cli_args: Vec<&OsStr> = vec![path.as_os_str()];
        if media {
            cli_args.extend([OsStr::new("--media"), dir.as_os_str()]);
        }
        let run = measure(args, &slug, &args.bin, &cli_args)?;
        if run["status"] != "exited" || run["code"] != code {
            return Err(format!("{name}: {run}, not an exit of {code}"));
        }
        measured.push(run);
        outputs.push(sha256(&stdout)?);
        if media {
            written = files_in(&dir)?;
        }
    }
    let text = fs::read_to_string(&stdout).map_err(|e| format!("{}: {e}", stdout.display()))?;
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    for line in text.lines() {
        let line: Value = serde_json::from_str(line).map_err(|e| format!("{name}: {e}"))?;
        let kind = line["line"].as_str().unwrap_or("?").to_owned();
        *kinds.entry(kind).or_default() += 1;
    }
    let stderr = fs::read_to_string(&stderr).map_err(|e| format!("{}: {e}", stderr.display()))?;
    let median = |field: &str| {
        let mut values: Vec<i64> = measured.iter().filter_map(|r| r[field].as_i64()).collect();
        values.sort_unstable();
        values[values.len() / 2]
    };
    let max_rss = measured
        .iter()
        .filter_map(|r| r["maxrss_kib"].as_i64())
        .max();
    Ok(json!({
        "input": name,
        "size": size(path)?,
        "media": media,
        "code": code,
        "runs": measured,
        "median_wall_ns": median("wall_ns"),
        "median_maxrss_kib": median("maxrss_kib"),
        "max_maxrss_kib": max_rss,
        "stdout_bytes": text.len(),
        "same_stdout_every_run": outputs.windows(2).all(|w| w[0] == w[1]),
        "lines": kinds,
        "stderr_lines": stderr.lines().collect::<Vec<_>>(),
        "written": written,
    }))
}

/// The files a run wrote into its media directory, and their bytes.
fn files_in(dir: &Path) -> Result<Value, String> {
    let (mut files, mut bytes) = (0u64, 0u64);
    if dir.exists() {
        for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            files += 1;
            bytes += entry.metadata().map_err(|e| e.to_string())?.len();
        }
    }
    Ok(json!({"files": files, "bytes": bytes}))
}

// ---- The run's facts ----------------------------------------------------------------------

fn sha256(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn size(path: &Path) -> Result<u64, String> {
    Ok(fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len())
}

fn cli(args: &Args) -> Result<Value, String> {
    let version = Command::new(&args.bin)
        .arg("--version")
        .output()
        .map_err(|e| format!("{}: {e}", args.bin.display()))?;
    Ok(json!({
        "version": String::from_utf8_lossy(&version.stdout).trim(),
        "commit": args.commit,
        "binary": args.bin.file_name().map(|n| n.to_string_lossy()),
        "build": "release, static musl, the check's target",
        "sha256": sha256(&args.bin)?,
        "size": size(&args.bin)?,
    }))
}

/// The CPU, its logical CPUs, the memory and the kernel, from /proc.
fn machine() -> Value {
    let read = |path: &str| fs::read_to_string(path).unwrap_or_default();
    let field = |text: &str, key: &str| {
        text.lines()
            .find(|line| line.starts_with(key))
            .and_then(|line| line.split_once(':'))
            .map(|(_, value)| value.trim().to_owned())
    };
    let cpuinfo = read("/proc/cpuinfo");
    let meminfo = read("/proc/meminfo");
    json!({
        "cpu": field(&cpuinfo, "model name"),
        "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).ok(),
        "memory": field(&meminfo, "MemTotal"),
        "kernel": read("/proc/sys/kernel/osrelease").trim(),
    })
}

/// The time now, ISO 8601 UTC to the second.
fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

// ---- The summary --------------------------------------------------------------------------

/// The tables, as Markdown, to stdout.
fn print_summary(results: &Value) {
    println!(
        "{NAME} {} ({})",
        results["cli"]["version"].as_str().unwrap_or("?"),
        results["cli"]["commit"].as_str().unwrap_or("?")
    );
    for (group, title) in [
        ("sections", "The committed sections against sections.json"),
        (
            "notebooks",
            "The committed notebooks against notebooks.json",
        ),
        ("content", "Content fields against content.json"),
    ] {
        let score = &results["recovery"][group];
        println!("\n### {title}\n");
        println!(
            "| Field kind | Expected | Recovered | Mismatched | Missing | Absent, as expected |"
        );
        println!("|---|---|---|---|---|---|");
        for row in items(&score["kinds"]) {
            let absent = match (row.get("absent_ok"), row.get("absent_expected")) {
                (Some(ok), Some(of)) => format!("{ok} of {of}"),
                _ => String::new(),
            };
            println!(
                "| {} | {} | {} | {} | {} | {absent} |",
                row["kind"].as_str().unwrap_or("?"),
                row["expected"],
                row["recovered"],
                row["mismatched"],
                row["missing"],
            );
        }
        for miss in items(&score["misses"]) {
            println!(
                "- {} {} {}: {}, expected {}, got {}",
                miss["file"].as_str().unwrap_or("?"),
                miss["at"].as_str().unwrap_or("?"),
                miss["kind"].as_str().unwrap_or("?"),
                miss["outcome"].as_str().unwrap_or("?"),
                miss["expected"],
                miss["got"]
            );
        }
    }
    println!("\n### Time and peak memory\n");
    println!(
        "rusage floor (/bin/true): {} KiB, {:.2} ms\n",
        results["rusage_floor"]["maxrss_kib"],
        results["rusage_floor"]["wall_ns"].as_f64().unwrap_or(0.0) / 1e6
    );
    println!(
        "| Input | Size | --media | Exit | Median wall | Median peak | Sections | Pages | Skipped | Stderr lines | Written |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for row in items(&results["timing"]) {
        let written = if row["media"] == true {
            format!(
                "{} files, {} B",
                row["written"]["files"], row["written"]["bytes"]
            )
        } else {
            String::new()
        };
        let count = |kind: &str| row["lines"].get(kind).cloned().unwrap_or(json!(0));
        println!(
            "| {} | {} | {} | {} | {} | {:.1} MiB | {} | {} | {} | {} | {written} |",
            row["input"].as_str().unwrap_or("?"),
            row["size"],
            if row["media"] == true { "yes" } else { "no" },
            row["code"],
            duration(row["median_wall_ns"].as_f64().unwrap_or(0.0)),
            row["median_maxrss_kib"].as_f64().unwrap_or(0.0) / 1024.0,
            count("section"),
            count("page"),
            count("skipped"),
            items(&row["stderr_lines"]).len(),
        );
    }
}

/// A wall time in nanoseconds, in milliseconds under 10 s, else in seconds.
fn duration(ns: f64) -> String {
    if ns < 1e10 {
        format!("{:.1} ms", ns / 1e6)
    } else {
        format!("{:.2} s", ns / 1e9)
    }
}
