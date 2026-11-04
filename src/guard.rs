//! The guard every onenote_parser call runs inside: the parser's error, or its panic, becomes a
//! reason.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::panic_text;

/// Why one parser call failed.
pub enum Failure {
    /// The parser's own error, which the caller may tell apart by kind.
    Error(onenote_parser::errors::Error),
    /// The parser panicked, with this message.
    Panic(String),
}

impl Failure {
    /// The reason a refusal or a skip gives.
    pub fn reason(&self) -> String {
        match self {
            Failure::Error(err) => err.to_string(),
            Failure::Panic(message) => format!("the parser panicked: {message}"),
        }
    }
}

/// Runs one parser call, turning its error or its panic into a [`Failure`].
pub fn guarded<T>(read: impl FnOnce() -> onenote_parser::errors::Result<T>) -> Result<T, Failure> {
    match catch_unwind(AssertUnwindSafe(read)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(Failure::Error(err)),
        Err(payload) => Err(Failure::Panic(panic_text(&*payload))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parser_panic_becomes_a_reason() {
        let failure = guarded::<()>(|| panic!("index out of bounds"))
            .err()
            .unwrap();
        assert_eq!(failure.reason(), "the parser panicked: index out of bounds");
    }
}
