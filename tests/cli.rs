//! The command line: arguments, usage errors, the input's sniffing, and the inputs refused with
//! exit 3.

mod common;

use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use common::{NAME, built, error, fixture, patched, run, run_to, scratch};

const USAGE: &str = "usage: spike-ai-pipeline-onenote [--media <dir>] <input>
       spike-ai-pipeline-onenote --version
";

/// A small section of each format: desktop (Interop's) and OneDrive (onenote.rs's).
const DESKTOP: &str = "interop/SUT/OneWithoutFileData.one";
const ONEDRIVE: &str = "onenote-rs/New Section 1.one";

#[test]
fn version_prints_the_bare_crate_version() {
    let out = run(&["--version"]);
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stdout, format!("{}\n", env!("CARGO_PKG_VERSION")));
    assert_eq!(out.stderr, "");
}

/// Each usage error exits 2 with its reason and the usage on stderr, and nothing on stdout.
#[test]
fn usage_errors_exit_2_with_usage() {
    let section = fixture(DESKTOP);
    let section = section.to_str().unwrap();
    let not_a_dir = fixture("interop/LICENSE");
    let not_a_dir = not_a_dir.to_str().unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&[], "no input"),
        (&[section, section], "more than one input"),
        (&["--frobnicate", section], "unknown option --frobnicate"),
        (
            &["--version", section],
            "--version takes no other arguments",
        ),
        (
            &[section, "--version"],
            "--version takes no other arguments",
        ),
        (&[section, "--media"], "--media needs a directory"),
        (&["--media", "", section], "--media needs a directory"),
        (
            &["--media", "a", "--media", "b", section],
            "--media is given more than once",
        ),
        (
            &["--media", not_a_dir, section],
            &format!("--media {not_a_dir} is not a directory"),
        ),
    ];
    for (args, reason) in cases {
        let out = run(args);
        assert_eq!(out.code, Some(2), "{args:?}");
        assert_eq!(out.stdout, "", "{args:?}");
        assert_eq!(out.stderr, format!("{NAME}: {reason}\n{USAGE}"), "{args:?}");
    }
}

/// --media is accepted before or after the input, and a directory that doesn't exist yet is
/// accepted.
#[test]
fn media_is_accepted_with_the_input() {
    let section = fixture(DESKTOP);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("media-unmade");
    let without = run(&[&section]);
    assert_eq!(without.code, Some(0), "{}", without.stderr);
    for args in [
        [section.as_os_str(), "--media".as_ref(), dir.as_os_str()],
        ["--media".as_ref(), dir.as_os_str(), section.as_os_str()],
    ] {
        let out = run(&args);
        assert_eq!(out.code, Some(0), "{}", out.stderr);
        assert_eq!(out.stdout, without.stdout);
    }
}

/// The arguments are read as OsString: an input whose name isn't UTF-8 reads, and the notebook
/// line names it with the invalid bytes replaced.
#[test]
fn an_input_whose_name_is_not_utf8_reads() {
    let name = OsStr::from_bytes(b"caf\xe9.one");
    let input = built(name, &std::fs::read(fixture(DESKTOP)).unwrap());
    let out = run(&[&input]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines()[0]["name"], "caf\u{fffd}");
}

/// The input is sniffed from its content, never its name: a section named as a ZIP or a text
/// file reads as the section, and text named as a section is refused as unknown content.
#[test]
fn the_input_is_sniffed_by_content_not_name() {
    let bytes = std::fs::read(fixture(ONEDRIVE)).unwrap();
    let intact = run(&[fixture(ONEDRIVE)]);
    for name in ["New Section 1.zip", "New Section 1.txt"] {
        let out = run(&[built(name, &bytes)]);
        assert_eq!(out.code, Some(0), "{name}: {}", out.stderr);
        // The section's id and its page lines are the section's. The notebook line names the
        // file's stem, and so does the section line when, as here, the section stores no
        // display name and onenote_parser takes the file's name.
        let (lines, want) = (out.lines(), intact.lines());
        assert_eq!(lines[1]["id"], want[1]["id"], "{name}");
        assert_eq!(lines[2..], want[2..], "{name}");
    }
    let text = built("text.one", b"not OneNote at all\n");
    let out = run(&[&text]);
    assert_eq!(out.code, Some(3));
    assert_eq!(
        out.stderr,
        error(&text, "not a OneNote section, notebook, or package")
    );
}

/// Each refused input exits 3 with one error line and nothing on stdout.
#[test]
fn refused_inputs_exit_3_before_any_output() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-such-section.one");
    let zip = built("download.one", b"PK\x03\x04\x14\x00\x00\x00\x08\x00");
    let empty_zip = built(
        "empty.zip",
        b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    );
    let empty_dir = scratch("empty-notebook");
    let sections_only = scratch("sections-only");
    std::fs::copy(fixture(DESKTOP), sections_only.join("a.one")).unwrap();
    let several = fixture("interop/Resources");
    let license = fixture("onenote-rs/LICENSE");
    let empty = built("empty.one", b"");
    let cases: &[(&Path, &str)] = &[
        (&missing, "No such file or directory (os error 2)"),
        (&zip, "a ZIP archive; extract the OneDrive download first"),
        (
            &empty_zip,
            "a ZIP archive; extract the OneDrive download first",
        ),
        (&empty_dir, "no .onetoc2 in the directory"),
        (&sections_only, "no .onetoc2 in the directory"),
        (
            &several,
            "more than one .onetoc2 in the directory: \
             AlternativePackaging.onetoc2, NoSection.onetoc2, Open Notebook.onetoc2",
        ),
        (&license, "not a OneNote section, notebook, or package"),
        (&empty, "not a OneNote section, notebook, or package"),
    ];
    for (input, reason) in cases {
        let out = run(&[input]);
        assert_eq!(out.code, Some(3), "{}", input.display());
        assert_eq!(out.stdout, "", "{}", input.display());
        assert_eq!(out.stderr, error(input, reason));
    }
}

/// A password-protected section is refused with the parser's bare reason, a known limit.
#[test]
fn a_password_protected_section_is_refused_with_the_parsers_reason() {
    let input = fixture("interop/SUT/Encryption.one");
    let out = run(&[&input]);
    assert_eq!(out.code, Some(3));
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr, error(&input, "Unexpected end of file"));
}

/// A parser panic is caught and refused with exit 3, never 4. The byte set here makes
/// onenote_parser add past its integer's range; the tests run the debug binary, whose overflow
/// checks make that a panic (a release build reads a different error there).
#[test]
fn a_parser_panic_is_refused_with_exit_3() {
    let input = patched(DESKTOP, 9348, 0xFF, "panic.one");
    let out = run(&[&input]);
    assert_eq!(out.code, Some(3));
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        error(&input, "the parser panicked: attempt to add with overflow")
    );
}

/// onenote_parser prints a dump of the file node to stdout as it fails on a node of the wrong
/// size; none of it reaches stdout, which a refusal leaves empty.
#[test]
fn the_parsers_own_printing_never_reaches_stdout() {
    let input = patched(DESKTOP, 1040, 0xFF, "node-size.one");
    let out = run(&[&input]);
    assert_eq!(out.code, Some(3));
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        error(
            &input,
            "Malformed OneNote file data: The size specified for this structure is incorrect. \
             Was 7, expected 27. Id: 0xff"
        )
    );
}

/// A .onetoc2, by its content whatever its name, a notebook directory and a cabinet are
/// recognised as a notebook or a package, which this version refuses as not read yet.
#[test]
fn notebooks_and_packages_are_recognised() {
    let toc = std::fs::read(fixture("onenote-rs/Open Notebook.onetoc2")).unwrap();
    let toc_as_section = built("Notebook.one", &toc);
    let notebook = fixture("onenote-rs/non-legacy");
    let cabinet = built("Notebook.onepkg", b"MSCF\0\0\0\0");
    let cases = [
        (toc_as_section.clone(), toc_as_section),
        (notebook.clone(), notebook.join("Open Notebook.onetoc2")),
        (cabinet.clone(), cabinet),
    ];
    for (input, named) in cases {
        let out = run(&[&input]);
        assert_eq!(out.code, Some(3), "{}", input.display());
        assert_eq!(out.stdout, "", "{}", input.display());
        assert_eq!(
            out.stderr,
            error(&named, "notebook and package inputs are not read yet")
        );
    }
}

/// A stdout that can't be written exits 1 with one error line, for a read and for --version.
#[test]
fn an_unwritable_stdout_exits_1() {
    for args in [fixture(DESKTOP).into_os_string(), "--version".into()] {
        let full = File::options().write(true).open("/dev/full").unwrap();
        let out = run_to(full, &[&args]);
        assert_eq!(out.code, Some(1), "{args:?}");
        assert_eq!(
            out.stderr,
            format!(
                "{NAME}: error: stdout can't be written: No space left on device (os error 28)\n"
            ),
            "{args:?}"
        );
    }
}
