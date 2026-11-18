// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The program a wrapped chat's line starts with** (ADR 0067 §2): the same profile, applied
//! by the system's own mechanism. On macOS, Seatbelt through `sandbox-exec`. On Linux,
//! purlis's own wrap (#1040): Landlock, a seccomp filter and the supervisor that judges each
//! connect and write by that profile, started as the hidden `purlis _sandbox` command.

use std::path::Path;

use super::Os;

/// Why a wrapped chat on Linux is not started: the line starts with purlis's own program, and
/// the place it opens did not say where that is.
pub const NO_PURLIS: &str = "purlis could not find its own program to apply the sandbox with";

/// The program and the arguments before `program` and `rest` for a chat wrapped in `profile` on
/// `os`, where purlis's own program is `purlis`; or why there are none.
pub fn line(
    os: Os,
    purlis: Option<&Path>,
    profile: String,
    program: String,
    rest: Vec<String>,
) -> Result<(String, Vec<String>), &'static str> {
    match os {
        Os::Linux => {
            let purlis = purlis.ok_or(NO_PURLIS)?;
            Ok((
                purlis.display().to_string(),
                purlis_linux_sandbox::line(&profile, program, rest),
            ))
        }
        _ => Ok((
            super::backend::SANDBOX_EXEC.to_owned(),
            [vec!["-p".to_owned(), profile, program], rest].concat(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_macos_the_profile_goes_to_sandbox_exec() {
        let (program, args) = line(
            Os::MacOs,
            None,
            "(version 1)\n".to_owned(),
            "codex".to_owned(),
            vec!["--a".to_owned()],
        )
        .expect("a line");
        assert_eq!(program, super::super::backend::SANDBOX_EXEC);
        assert_eq!(args, ["-p", "(version 1)\n", "codex", "--a"]);
    }

    #[test]
    fn on_linux_the_line_starts_with_purlis_s_hidden_command() {
        let (program, args) = line(
            Os::Linux,
            Some(Path::new("/opt/purlis/bin/purlis")),
            "(version 1)\n".to_owned(),
            "codex".to_owned(),
            vec!["--a".to_owned()],
        )
        .expect("a line");
        assert_eq!(program, "/opt/purlis/bin/purlis");
        assert_eq!(
            args,
            [
                "_sandbox",
                "--profile",
                "(version 1)\n",
                "--",
                "codex",
                "--a"
            ]
        );
    }

    #[test]
    fn on_linux_without_purlis_s_own_program_nothing_starts() {
        assert_eq!(
            line(
                Os::Linux,
                None,
                String::new(),
                "codex".to_owned(),
                Vec::new()
            ),
            Err(NO_PURLIS)
        );
    }
}
