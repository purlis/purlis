// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What kind of failure a command's is** (#5862, ADR 0120): purlis failed inside itself, it
//! refused on purpose, or the command line asked for something it cannot mean. Only the first
//! exits [`INTERNAL_EXIT`], the code purlis's own hook drafts a Report of
//! ([`purlis_core::caught`]); [`CliError::exit`] is the one place that says so.
//!
//! **A `String` is a refusal.** It is what every command's error was before this type, and
//! `From<String>` keeps that meaning, so a command moved over keeps every refusal it had. An
//! [`std::io::Error`] that reaches `?` unhandled is internal: an error the command means the
//! person to act on (a file they named that is not there) is said as a refusal before it gets
//! that far.

use std::io::Write as _;
use std::process::ExitCode;

use purlis_core::caught::INTERNAL_EXIT;

/// A command's failure, by kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// purlis failed inside itself: an unhandled I/O error on its own state, a state the
    /// command has no arm for, an answer it did not ask for.
    Internal(String),
    /// purlis declined on purpose: a guard, a policy, something the person can change.
    Refusal(String),
    /// The command line asked for something it cannot mean.
    Usage(String),
}

impl CliError {
    /// A failure inside purlis, `why` said after the mark.
    pub fn internal(why: impl std::fmt::Display) -> Self {
        Self::Internal(why.to_string())
    }

    /// The status this failure ends a command with: [`INTERNAL_EXIT`] for an internal one, 1
    /// for a refusal and a usage error. The one place the internal kind becomes its code.
    pub fn code(&self) -> u8 {
        match self {
            Self::Internal(_) => INTERNAL_EXIT,
            Self::Refusal(_) | Self::Usage(_) => 1,
        }
    }

    /// Says this failure on standard error, as a command's last line, and gives its status.
    pub fn exit(&self) -> ExitCode {
        // Never `eprintln!`: a print to a closed stderr panics, and a panic is a second failure.
        let _ = writeln!(std::io::stderr(), "{self}");
        ExitCode::from(self.code())
    }

    /// The words of a refusal or a usage error, for a command that says them its own way, or
    /// this failure back where it is internal, so `?` carries it to [`CliError::exit`].
    pub fn refused(self) -> Result<String, Self> {
        match self {
            Self::Refusal(why) | Self::Usage(why) => Ok(why),
            internal @ Self::Internal(_) => Err(internal),
        }
    }

    /// The error a purlis tool answers for this failure: an internal one carries the mark its
    /// hook drafts a Report of ([`purlis_core::caught::tool_failed_inside`]), any other the
    /// words alone.
    pub fn for_a_tool(self) -> String {
        match self {
            Self::Internal(why) => purlis_core::caught::tool_failed_inside(why),
            Self::Refusal(why) | Self::Usage(why) => why,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal(why) => write!(f, "purlis: internal failure: {why}"),
            Self::Refusal(why) | Self::Usage(why) => write!(f, "purlis: {why}"),
        }
    }
}

impl From<String> for CliError {
    fn from(why: String) -> Self {
        Self::Refusal(why)
    }
}

impl From<&str> for CliError {
    fn from(why: &str) -> Self {
        Self::Refusal(why.to_owned())
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_internal_failure_exits_with_the_internal_code() {
        assert_eq!(CliError::internal("a state").code(), INTERNAL_EXIT);
        assert_eq!(CliError::Refusal("no".into()).code(), 1);
        assert_eq!(CliError::Usage("bad".into()).code(), 1);
    }

    #[test]
    fn a_string_stays_a_refusal_and_an_unhandled_io_error_is_internal() {
        assert_eq!(
            CliError::from(String::from("no")),
            CliError::Refusal("no".into())
        );
        let io = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(matches!(CliError::from(io), CliError::Internal(_)));
    }

    #[test]
    fn a_tool_marks_an_internal_failure_and_nothing_else() {
        assert!(
            CliError::internal("x")
                .for_a_tool()
                .starts_with(purlis_core::caught::TOOL_FAILED_INSIDE)
        );
        assert_eq!(CliError::Refusal("no".into()).for_a_tool(), "no");
    }
}
