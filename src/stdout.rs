//! stdout, kept for the lines alone. onenote_parser prints to stdout as it fails on some
//! malformed desktop sections (a multi-line "Incorrect structure size" dump of the file node),
//! which would put text that isn't JSON among the lines, or before a refusal that must leave
//! stdout empty. So before the parser runs, the CLI takes a duplicate of file descriptor 1 for
//! its own writer and points descriptor 1 at /dev/null, where anything printed through Rust's
//! stdout then goes.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd};

use crate::Fault;

/// Takes stdout for the lines: a file on the descriptor stdout was, with descriptor 1 left
/// pointing at /dev/null.
pub fn take() -> Result<File, Fault> {
    let out = io::stdout()
        .as_fd()
        .try_clone_to_owned()
        .map_err(Fault::Output)?;
    let null = File::options()
        .write(true)
        .open("/dev/null")
        .map_err(|err| Fault::Internal(format!("/dev/null can't be opened: {err}")))?;
    // SAFETY: dup2 takes two descriptors and touches no memory; both are open, `null` for the
    // length of the call, and descriptor 1 is the process's own to replace.
    if unsafe { libc::dup2(null.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
        let err = io::Error::last_os_error();
        return Err(Fault::Internal(format!(
            "stdout can't be pointed at /dev/null: {err}"
        )));
    }
    Ok(File::from(out))
}
