//! spike-ai-pipeline-onenote: OneNote sections, packages, and notebooks to a JSON model.
//!
//! It reads one input, sniffed by content, and writes JSON Lines of schema "1" to stdout:
//! nothing else goes there. Warnings and errors are single lines on stderr. Exit codes:
//! 0 read, even with records skipped; 1 output that can't be written, to stdout or to the
//! media directory; 2 a usage error; 3 an input refused before any output; 4 a fault of the
//! CLI's own.

mod content;
mod guard;
mod ink;
mod jsonl;
mod math;
mod media;
mod model;
mod section;
mod sniff;
mod stdout;

use std::any::Any;
use std::ffi::OsString;
use std::io::{self, BufWriter, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jsonl::Lines;
use media::Media;
use sniff::Input;

const NAME: &str = "spike-ai-pipeline-onenote";

const USAGE: &str = "usage: spike-ai-pipeline-onenote [--media <dir>] <input>
       spike-ai-pipeline-onenote --version";

/// Why a run stopped short of exit 0.
pub enum Fault {
    /// The arguments: exit 2, with usage.
    Usage(String),
    /// The input: exit 3, with one reason line and nothing on stdout.
    Refused(String),
    /// stdout couldn't be written: exit 1.
    Output(io::Error),
    /// The media directory, or a file in it, couldn't be written: exit 1, with this reason.
    Media(String),
    /// A fault of the CLI's own: exit 4.
    Internal(String),
}

/// The parsed command line.
enum Command {
    Version,
    Read(Args),
}

struct Args {
    input: PathBuf,
    media: Option<PathBuf>,
}

fn main() -> ExitCode {
    // Panics are reported once, as an error line, never by the default hook.
    panic::set_hook(Box::new(|_| {}));
    // args_os: a path needn't be UTF-8, and std::env::args panics on one that isn't.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| run(&args)))
        .unwrap_or_else(|payload| Err(Fault::Internal(panic_text(&*payload))));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fault::Output(err)) => fail(1, &format!("stdout can't be written: {err}")),
        Err(Fault::Media(reason)) => fail(1, &reason),
        Err(Fault::Usage(reason)) => {
            eprintln!("{NAME}: {reason}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Fault::Refused(reason)) => fail(3, &reason),
        Err(Fault::Internal(reason)) => fail(4, &format!("internal fault: {reason}")),
    }
}

fn fail(code: u8, reason: &str) -> ExitCode {
    eprintln!("{NAME}: error: {reason}");
    ExitCode::from(code)
}

fn run(args: &[OsString]) -> Result<(), Fault> {
    let args = match parse(args)? {
        Command::Version => {
            // The bare version, which the release compares with its tag's. Written, not
            // println!ed, so a stdout that can't take it exits 1 rather than panicking.
            return writeln!(io::stdout(), "{}", env!("CARGO_PKG_VERSION")).map_err(Fault::Output);
        }
        Command::Read(args) => args,
    };
    let path = &args.input;
    let input = sniff::sniff(path)?;
    // From here, descriptor 1 is /dev/null and the lines go to the descriptor stdout was.
    let mut out = Lines::new(BufWriter::new(stdout::take()?));
    let mut media = Media::new(args.media);
    match input {
        Input::OneStore => section::read(path, &mut media, &mut out)?,
        Input::Directory(toc) => return Err(notebook_unread(&toc)),
        Input::Package => return Err(notebook_unread(path)),
    }
    out.flush().map_err(Fault::Output)
}

/// Writes one warning line: `place` names the record, from its section down, and `what` says
/// what happened to it.
pub fn warn(place: &str, what: &str) {
    eprintln!("{NAME}: warning: {place}: {what}");
}

/// The refusal of a notebook, a notebook directory, or a package, which aren't read yet.
pub fn notebook_unread(path: &Path) -> Fault {
    Fault::Refused(format!(
        "{}: notebook and package inputs are not read yet",
        path.display()
    ))
}

/// Parses the arguments by hand: `--version` alone, or one input and an optional
/// `--media <dir>`, in any order.
fn parse(args: &[OsString]) -> Result<Command, Fault> {
    if let [flag] = args
        && flag == "--version"
    {
        return Ok(Command::Version);
    }
    let usage = |reason: &str| Err(Fault::Usage(reason.to_owned()));
    let mut input = None;
    let mut media = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--media") => {
                let Some(dir) = rest.next().filter(|dir| !dir.is_empty()) else {
                    return usage("--media needs a directory");
                };
                if media.replace(PathBuf::from(dir)).is_some() {
                    return usage("--media is given more than once");
                }
            }
            Some("--version") => return usage("--version takes no other arguments"),
            Some(option) if option.starts_with('-') && option != "-" => {
                return usage(&format!("unknown option {option}"));
            }
            _ => {
                if input.replace(PathBuf::from(arg)).is_some() {
                    return usage("more than one input");
                }
            }
        }
    }
    let Some(input) = input else {
        return usage("no input");
    };
    if let Some(dir) = &media
        && dir.exists()
        && !dir.is_dir()
    {
        return usage(&format!("--media {} is not a directory", dir.display()));
    }
    Ok(Command::Read(Args { input, media }))
}

/// The message a panic was raised with.
pub fn panic_text(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_owned())
}
