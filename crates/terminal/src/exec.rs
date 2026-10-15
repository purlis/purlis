// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the system starts a program with (#1425), and a start that is longer than that, said in
//! plain words before the system refuses it with `E2BIG`. A terminal's program is started
//! within it, and so is every program a sandbox wraps (`purlis_core::sandbox::seatbelt`).

/// Why a chat could not be started: its program's words and environment together are longer
/// than the system starts a program with ([`fits_exec`], #1425), which it would refuse with
/// `E2BIG`.
pub const TOO_LONG: &str = "the chat's first message, its other words and its environment \
                            together are longer than this system starts a program with";

/// What a person can do about [`TOO_LONG`]: the start is what is too long, so a shorter one.
pub const TOO_LONG_WAY_OUT: &str =
    "Start it with a shorter first message, and send the rest once it has started.";

/// What the system starts a program with (#1425): at most `total` bytes of words and
/// environment together, and, where it holds one, at most `word` bytes in any one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exec {
    pub total: usize,
    pub word: Option<usize>,
}

impl Exec {
    /// This system's: macOS's `ARG_MAX` (1 MiB, fixed in its kernel); on Linux, the 2 MiB a
    /// default 8 MiB stack gives (a quarter of it) and the 128 KiB any one string may be
    /// (`MAX_ARG_STRLEN`). Each less a margin for what the system adds itself (the program's
    /// own path, macOS's `apple` strings).
    pub const HERE: Self = if cfg!(target_os = "macos") {
        Self {
            total: 1024 * 1024 - EXEC_MARGIN,
            word: None,
        }
    } else {
        Self {
            total: 2 * 1024 * 1024 - EXEC_MARGIN,
            word: Some(128 * 1024 - 1),
        }
    };
}

/// What [`Exec::HERE`] leaves for what the system adds to a start itself.
const EXEC_MARGIN: usize = 16 * 1024;

/// How many bytes starting `program` with `args` and `env` takes of what the system starts a
/// program with: each string with its closing NUL and a pointer to it, and each list's closing
/// null pointer. An environment entry is one `KEY=value` string.
pub fn exec_size(program: &str, args: &[String], env: &[(String, String)]) -> usize {
    const POINTER: usize = std::mem::size_of::<usize>();
    let words = std::iter::once(program.len())
        .chain(args.iter().map(String::len))
        .chain(env.iter().map(|(key, value)| key.len() + 1 + value.len()));
    words.fold(2 * POINTER, |sum, len| {
        sum.saturating_add(len).saturating_add(1 + POINTER)
    })
}

/// Whether starting `program` with `args` and `env`, the whole environment it starts with,
/// fits `exec` ([`Exec::HERE`] for this system): [`TOO_LONG`] where it does not, said in plain
/// words before the system refuses the start with `E2BIG` (#1425). Refused rather than
/// started another way: handing the first message over by a file is a harness's protocol,
/// not purlis's to change (D-1425-2).
pub fn fits_exec(
    program: &str,
    args: &[String],
    env: &[(String, String)],
    exec: Exec,
) -> Result<(), &'static str> {
    if let Some(most) = exec.word {
        let longest = args
            .iter()
            .map(String::len)
            .chain(env.iter().map(|(key, value)| key.len() + 1 + value.len()))
            .chain(std::iter::once(program.len()))
            .max()
            .unwrap_or(0);
        if longest > most {
            return Err(TOO_LONG);
        }
    }
    if exec_size(program, args, env) > exec.total {
        return Err(TOO_LONG);
    }
    Ok(())
}
