// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which workspace and which persona a command acts on when nobody said.
//!
//! Until this module the Rust `charter` had no notion of either: every command that needed a
//! workspace took `-w` and **required** it, so `charter ws remember "…"` — and, once M2.2
//! lands it, `charter recall` with no flags, which is how a harness calls it at session start
//! — was refused with a usage error rather than run. Python resolves both, and its ladders
//! are the contract `docs/plane-format.md` records; this is that contract in Rust.
//!
//! # The workspace ladder
//!
//! `charter/workspace.py:589` (`chosen`) plus `resolve`'s fallback under it, and
//! `docs/plane-format.md`'s "Resolution order":
//!
//! | # | Rung | Where it comes from |
//! |---|------|---------------------|
//! | 1 | [`WorkspaceRung::Flag`] | `-w` / `--workspace` |
//! | 2 | [`WorkspaceRung::Environment`] | `$PURLIS_WORKSPACE` (or `$CHARTER_WORKSPACE`) |
//! | 3 | [`WorkspaceRung::Cwd`] | the tree the caller is standing in |
//! | 4 | [`WorkspaceRung::SessionPointer`] | `.charter/sessions/<sid>.workspace` |
//! | — | *the frame's launch record* | **left out on purpose — see below** |
//! | 5 | [`WorkspaceRung::TerminalPointer`] | `.charter/terminals/<tid>.workspace` |
//! | 6 | [`WorkspaceRung::DeclaredDefault`] | `workspaces/.default` |
//! | 7 | [`WorkspaceRung::PlaneDefault`] | `[workspace] default` in `charter.toml` |
//! | 8 | [`WorkspaceRung::BuiltIn`] | the literal `default` |
//!
//! The cwd sits above the pointers because it cannot be wrong: a workspace's trees live at
//! paths that name the workspace, so being inside one is not a hint, it is the fact. The
//! pointers are for the case with no tree to stand in.
//!
//! **The frame's launch record (`.charter/frame/<fid>/workspace`) is deliberately absent**,
//! and this is the one place the two implementations answer differently on purpose.
//! `docs/plane-format.md` rules that `.charter/frame/**` is the tmux frame's and that the app
//! "neither reads nor writes there" — the app replaces that frame rather than inheriting its
//! state. A rung nothing on this side can ever set is a rung nobody can reason about, so it
//! is named here, left out, and pinned by the differential scenario
//! `workspace-the-frames-launch-record-is-a-rung-in-python-and-not-here` — which FAILS the
//! day the two stop differing, so the note cannot outlive the divergence it records.
//!
//! **What that costs, because a decision without its consequence is half a record.** The two
//! can differ in exactly one state: a chat the tmux frame launched in which nobody has run
//! `charter workspace use`, so the session pointer is absent and a frame record is present.
//! There Python reads the launch workspace and this falls through to the terminal pointer and
//! below — `workspace current` answers `default` while the frame's own panels still show the
//! workspace they were launched for, a `ws remember` lands under `workspaces/default/`, and
//! `recall` opens the session on another workspace's memory. And the rung it falls to is the
//! one charter deliberately outranked: `terminal_id` inside a frame reads a pane charter
//! created, or the terminal the whole frame inherited from, neither of which is the chat.
//! One `charter workspace use` closes it for that chat, permanently, by writing the rung
//! above. The whole of it — the ruling, the cost and the alternative not taken — is
//! `docs/adr/0032-the-rust-charter-does-not-read-the-frames-launch-record.md` in
//! `diazoxide/charter-plane`, where every ADR from 0025 lives (charter-app#74).
//!
//! # The persona ladder
//!
//! `charter/persona.py:1379` (`_resolved`), seven rungs and then nothing:
//!
//! | # | Rung | Where it comes from |
//! |---|------|---------------------|
//! | 1 | [`PersonaRung::Flag`] | `--persona` |
//! | 2 | [`PersonaRung::Environment`] | `$PURLIS_PERSONA` (or `$CHARTER_PERSONA`) |
//! | 3 | [`PersonaRung::SessionPointer`] | `.charter/sessions/<sid>.persona` |
//! | 4 | [`PersonaRung::TerminalPointer`] | `.charter/terminals/<tid>.persona` |
//! | 5 | [`PersonaRung::ActiveFile`] | `.charter/active-persona` |
//! | 6 | [`PersonaRung::PlaneDefault`] | `[persona] default` in `charter.toml` |
//! | 7 | [`PersonaRung::CommittedDefault`] | `personas/.default` |
//! | — | [`PersonaRung::Nothing`] | no persona, which is a real answer |
//!
//! The two committed rungs name only a persona that EXISTS; a rung above them naming one that
//! does not still wins, and the session has no persona rather than the plane's default. That
//! asymmetry is charter's (`charter/persona.py:1038`) and is what makes a pointer left behind
//! by `persona remove` visible instead of silently replaced.
//!
//! # Every name read off disk is checked before it is joined onto a path
//!
//! `workspace_dir()` is `workspaces/` joined with whatever resolved, so `../../esc` in a rung
//! reaches outside the plane — charter#442, on the rungs that were not gated. Here **every**
//! rung is name-checked, the two local pointer files included.
//!
//! **That is one step stricter than Python**, deliberately and with the divergence declared:
//! `workspace.chosen` and `persona._resolved` read the session and terminal pointers through
//! `_read`/`_read_pointer` and hand the value back unchecked, while their own public twins
//! (`workspace.for_session`, `persona.for_session`) name-check and say in their docstrings
//! that every rung does. Filed upstream as charter#1146. Nothing here depends on that being
//! fixed: a pointer charter itself wrote always passes.

use std::path::{Path, PathBuf};

use purlis_skills::fence::py_strip;

use crate::contain;

/// `$CHARTER_WORKSPACE` — the per-session pin a launcher hands a chat.
///
/// The app sets it on every chat it starts in a workspace (SI-1), so that chat never has to
/// be asked which workspace it is in.
pub const WORKSPACE_ENV: &str = "PURLIS_WORKSPACE";

/// `$CHARTER_PLANE_ROOT_SESSION` — the pin a launcher hands a chat it started at the **plane
/// root**: in no workspace, on purpose (SI-1).
///
/// **A variable of its own, and not a value of [`WORKSPACE_ENV`].** Anything that reads
/// `$CHARTER_WORKSPACE` — an older `charter`, an extension's program, a script — takes its
/// value as a workspace name and joins it onto `workspaces/`. A sentinel there is a name
/// something will one day create a directory for; a second variable is one an older reader
/// simply does not see, and that reader then answers as it always did.
///
/// **Only [`PLANE_ROOT_ON`] turns it on** ([`at_plane_root`]). Any other value is a chat the
/// ladder answers as it always has — the failure that changes nothing.
pub const PLANE_ROOT_ENV: &str = "PURLIS_PLANE_ROOT_SESSION";

/// The one value of [`PLANE_ROOT_ENV`] that means "this chat is at the plane root".
pub const PLANE_ROOT_ON: &str = "1";

/// `$CHARTER_SESSION_ID` — what the pointer rungs are keyed on, and the variable a LAUNCHER
/// sets so that every process it starts answers [`session_id`] identically.
///
/// Named here rather than spelled out at each side, because the reader is this module and the
/// writer is whatever launched the chat — the tmux frame in Python (`-e
/// CHARTER_SESSION_ID=<chat id>`), and `app/src-tauri/src/sessions.rs` here. Two string
/// literals in two crates is how the app came to set `CHARTER_CHAT` and nothing charter reads
/// (charter-app#63).
pub const SESSION_ID_ENV: &str = "PURLIS_SESSION_ID";

/// `$CLAUDE_CODE_SESSION_ID` — the harness's own id for the CONVERSATION, one rung under
/// [`SESSION_ID_ENV`].
///
/// A conversation is not a chat: `/clear` ends one and starts another inside the same chat,
/// in the same terminal, under the same program. Anything keyed on this is therefore keyed on
/// something that changes under a chat that did not move, which is charter-app#63 exactly.
pub const CONVERSATION_ENV: &str = "CLAUDE_CODE_SESSION_ID";

/// `$CHARTER_PERSONA` — the same, for the identity a chat adopts.
pub const PERSONA_ENV: &str = "PURLIS_PERSONA";

/// How the environment rung of each ladder is labelled: by the variable it reads, under every
/// name it is read by (`$PURLIS_PERSONA or $CHARTER_PERSONA`). The purlis name decides when it
/// is set and the old one when it is not, so a label naming one would send someone to unset a
/// variable that is not the one deciding (#1344, #1284).
static WORKSPACE_ENV_LABEL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    crate::envvar::in_words(WORKSPACE_ENV, |n| format!("${n}"), " or ")
});
static PERSONA_ENV_LABEL: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| crate::envvar::in_words(PERSONA_ENV, |n| format!("${n}"), " or "));

/// The workspace every plane falls back to, whether or not its directory is there.
pub const BUILT_IN_WORKSPACE: &str = "default";

/// The most a rung's file may hold before charter stops reading it — `contain.MAX_BYTES`.
///
/// A rung holds one name, so a cap of a few hundred bytes would do; it is charter's 1 MiB
/// because the two implementations must refuse the same files, and a file of padding with a
/// legal name inside it is legal to Python.
const MAX_RUNG_BYTES: u64 = 1_048_576;

// --------------------------------------------------------------------------------------- //
// who is asking                                                                             //
// --------------------------------------------------------------------------------------- //

/// The two ids the pointer rungs are keyed on — `charter/session.py`, which owns both.
///
/// Held together because "who is this session" and "which pane is it in" are the two halves
/// of one question, and charter's own module exists because that question once had three
/// disagreeing answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ids {
    /// This session's id, or `None` when there is not one. Absence is representable on
    /// purpose: charter's `NO_SESSION` sentinel is a SHARED key, so every session without an
    /// id would write into one bucket.
    pub session: Option<String>,
    /// A stable id for the current terminal PANE, or `None`.
    pub terminal: Option<String>,
}

impl Ids {
    /// Both ids, read from this process's environment.
    pub fn from_env() -> Self {
        Self {
            session: session_id(&from_env),
            terminal: terminal_id(&from_env, tty_name),
        }
    }

    /// Both ids from a named environment, for a test and for a process asking on behalf of
    /// another. Never reads the real environment and never asks for a tty.
    pub fn of(env: &dyn Fn(&str) -> Option<String>) -> Self {
        Self {
            session: session_id(env),
            terminal: terminal_id(env, || None),
        }
    }
}

fn from_env(name: &str) -> Option<String> {
    crate::envvar::var(name)
}

/// `charter/session.py:current` — `$CHARTER_SESSION_ID`, then `$CLAUDE_CODE_SESSION_ID`.
///
/// **The second rung is not dead code and was twice documented as such.** Measured against
/// Claude Code through a real `statusLine` command, the environment arrives intact and
/// carries `CLAUDE_CODE_SESSION_ID`; measured again here from an ordinary tool call, it is
/// still there. Inside a charter frame `$CHARTER_SESSION_ID` holds the FRAME's id and
/// SHADOWS it, which is what makes every process in that frame answer identically.
///
/// The value becomes a filename, so everything outside `[A-Za-z0-9._-]` is **deleted** —
/// charter's `_SAFE.sub("", …)`, and not the `-` substitution [`terminal_id`] uses. Two
/// different rules, kept apart because charter keeps them apart.
///
/// Public because [`crate::trace::bucket`] is the same question with a sentinel instead of
/// `None`, and it had its own copy of this until M2.9 put the two side by side.
pub fn session_id(env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    // `or` on the RAW value, exactly as Python's `a or b` does: a `$CHARTER_SESSION_ID` of
    // whitespace is truthy, so it is taken and then sanitised to nothing — it does NOT fall
    // through to Claude Code's id. Stripping before the choice would change which variable
    // decides.
    let raw = env(SESSION_ID_ENV)
        .filter(|v| !v.is_empty())
        .or_else(|| env(CONVERSATION_ENV).filter(|v| !v.is_empty()))?;
    let id: String = py_strip(&raw)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    (!id.is_empty()).then_some(id)
}

/// Environment variables that identify ONE PANE — one shell, so at most one session.
///
/// `WINDOWID` is deliberately absent and used to sit in this list. It identifies a *window*,
/// and a window holds many tabs and splits, so every session in it wrote and read one
/// another's pointer: two sessions in one window, one runs `ws use user-reporting`, and the
/// other silently moves with it. An id that is wrong in the sharing direction is worse than
/// no id.
const PANE_ID_VARS: [&str; 4] = ["TERM_SESSION_ID", "TMUX_PANE", "STY", "SSH_TTY"];

/// `charter/session.py:terminal` — the pane, which survives closing and reopening the harness.
///
/// The `ttyname(0)` fallback is charter's last resort and is ported with it, because it is the
/// rung that fires **under the app**: the app gives each chat a pty of its own and sets none
/// of the four variables above, so this is the only thing that answers there. It answers with
/// a device name the kernel recycles, which is the sharing hazard `WINDOWID` was removed for,
/// one layer down — see the PR for what the app should write instead.
fn terminal_id(
    env: &dyn Fn(&str) -> Option<String>,
    tty: fn() -> Option<String>,
) -> Option<String> {
    let raw = PANE_ID_VARS
        .iter()
        .find_map(|name| env(name).filter(|v| !v.is_empty()))
        .or_else(tty)?;
    let id: String = py_strip(&raw)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    (!id.is_empty()).then_some(id)
}

/// The controlling terminal's device name, when stdin is one; off unix, never.
///
/// One function with the platform inside it rather than a `#[cfg(not(unix))]` twin: a twin
/// no unix build compiles is code the mutation run changes and no test there can run, so it
/// reported every change to it as untested.
fn tty_name() -> Option<String> {
    #[cfg(unix)]
    {
        rustix::termios::ttyname(std::io::stdin(), Vec::new())
            .ok()
            .and_then(|name| name.into_string().ok())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

// --------------------------------------------------------------------------------------- //
// the workspace ladder                                                                      //
// --------------------------------------------------------------------------------------- //

/// Which rung of [`workspace`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRung {
    /// `-w` / `--workspace`.
    Flag,
    /// `$CHARTER_WORKSPACE`.
    Environment,
    /// The tree the caller is standing in.
    Cwd,
    /// `.charter/sessions/<sid>.workspace`.
    SessionPointer,
    /// `.charter/terminals/<tid>.workspace`.
    TerminalPointer,
    /// `workspaces/.default`, nominated by `charter workspace default`.
    DeclaredDefault,
    /// `[workspace] default` in `charter.toml`.
    PlaneDefault,
    /// The literal `default`, which is nobody's decision.
    BuiltIn,
}

impl WorkspaceRung {
    /// How charter names this rung on a status line — `charter/workspace.py:652` (`source`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Flag => "--workspace",
            Self::Environment => WORKSPACE_ENV_LABEL.as_str(),
            Self::Cwd => "cwd",
            Self::SessionPointer => "session",
            Self::TerminalPointer => "terminal",
            Self::DeclaredDefault => "declared default",
            Self::PlaneDefault | Self::BuiltIn => "default (nothing selected)",
        }
    }
}

/// The sentence `charter status` prints for "where did this workspace come from" —
/// `charter/workspace.py:source`, whose last two lines [`WorkspaceRung::label`] cannot
/// reach on its own.
///
/// **The last rung says WHY nothing answered, not just that nothing did.** A shell with no
/// pane id has no terminal pointer to fall back on, so every session in it starts on
/// `default` however many times the operator picks — and the operator's own complaint was
/// "why are you in default workspace again?", asked of a surface that asserted an answer
/// with no reason (ADR 0013's second rule). The two spellings are told apart by the pane id
/// and by nothing else, which is exactly what Python's `if not _terminal_id()` asks.
pub fn workspace_source(ids: &Ids, rung: WorkspaceRung) -> String {
    match rung {
        WorkspaceRung::PlaneDefault | WorkspaceRung::BuiltIn if ids.terminal.is_none() => {
            "default (no pane id — nothing persists between sessions)".to_string()
        }
        other => other.label().to_string(),
    }
}

/// The workspace a command acts on, and which rung said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWorkspace {
    pub name: String,
    pub rung: WorkspaceRung,
}

/// Everything the ladders read that is not a file: the flags this invocation carries and the
/// environment they sit in.
///
/// Passed rather than read, so a test drives every rung and so the one caller that is not the
/// session being described — a renderer answering for a payload's cwd — can say whose
/// directory the cwd rung should read.
pub struct Asking<'a> {
    /// The plane being acted on.
    pub root: &'a Path,
    /// Whose working directory the cwd rung reads.
    pub cwd: &'a Path,
    /// `-w` / `--persona` as typed, or `None`.
    pub flag: Option<&'a str>,
    /// The session and pane this is running in.
    pub ids: &'a Ids,
    /// `$CHARTER_WORKSPACE` / `$CHARTER_PERSONA`, exactly as the environment holds it.
    pub env: Option<&'a str>,
}

/// The active workspace, by precedence, with the built-in underneath — Python's `resolve`.
pub fn workspace(asking: &Asking) -> ActiveWorkspace {
    if let Some(chosen) = chosen_workspace(asking) {
        return chosen;
    }
    // `[workspace] default` and the literal `default` are one rung to charter — `source` says
    // "default (nothing selected)" for both — and two here, because only one of them is a
    // line somebody wrote in a committed file, and a caller deciding whether to ask has to be
    // able to tell them apart.
    match declared_plane_default(asking.root) {
        Some(name) => ActiveWorkspace {
            name,
            rung: WorkspaceRung::PlaneDefault,
        },
        None => ActiveWorkspace {
            name: BUILT_IN_WORKSPACE.to_string(),
            rung: WorkspaceRung::BuiltIn,
        },
    }
}

/// The workspace something actually **chose**, or `None` when every rung came back empty —
/// Python's `chosen`, which is `resolve` minus its last rung.
///
/// The difference is not cosmetic: `None` means nobody decided, which is the launch worth
/// interrupting with a picker. One ladder asked twice, never two ladders that agree today.
pub fn chosen_workspace(asking: &Asking) -> Option<ActiveWorkspace> {
    if let Some(picked) = picked_workspace(asking) {
        return Some(picked);
    }
    declared_default_workspace(asking.root).map(|name| ActiveWorkspace {
        name,
        rung: WorkspaceRung::DeclaredDefault,
    })
}

/// The rungs that speak for THIS session — `-w`, `$CHARTER_WORKSPACE`, the tree it stands in,
/// its own pointer and its terminal's — or `None` when none of them does. Everything under
/// them is the plane's default, which speaks for nobody in particular.
fn picked_workspace(asking: &Asking) -> Option<ActiveWorkspace> {
    let at = |name: String, rung| Some(ActiveWorkspace { name, rung });
    if let Some(flag) = asking.flag.filter(|f| !f.is_empty()) {
        return at(flag.to_string(), WorkspaceRung::Flag);
    }
    if let Some(named) = asking.env.map(py_strip).filter(|v| !v.is_empty()) {
        return at(named.to_string(), WorkspaceRung::Environment);
    }
    if let Some(here) = workspace_of_tree(asking.root, asking.cwd) {
        return at(here, WorkspaceRung::Cwd);
    }
    if let Some(name) = pointer(
        asking.root,
        "sessions",
        asking.ids.session.as_deref(),
        "workspace",
    )
    .filter(|n| contain::workspace_name_ok(n))
    {
        return at(name, WorkspaceRung::SessionPointer);
    }
    if let Some(name) = pointer(
        asking.root,
        "terminals",
        asking.ids.terminal.as_deref(),
        "workspace",
    )
    .filter(|n| contain::workspace_name_ok(n))
    {
        return at(name, WorkspaceRung::TerminalPointer);
    }
    None
}

/// Why a session is at the **plane root**, in no workspace (SI-1). See [`plane_root`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneRoot {
    /// Its launcher said so: [`PLANE_ROOT_ENV`] is [`PLANE_ROOT_ON`]. The app's word for where
    /// it started the chat, so the chat stays there: `charter workspace use` does not move it.
    Launched,
    /// It stands in the plane, outside every workspace, and nothing chose one for it. Nothing
    /// pins it there either, so `charter workspace use` moves it as it always has.
    Standing,
}

/// Whether the session `asking` describes is at the plane root, and why — or `None` when it
/// is in a workspace, and the ladder ([`workspace`]) says which.
///
/// It is asked BEFORE the ladder, by every caller that would otherwise act on the ladder's
/// answer, because the ladder always ends on a name (`default`) and a session at the plane
/// root is in no workspace at all. **The one source of truth for it**: the CLI, the hooks and
/// the footer all ask here, so no two of them can put one session in two places.
///
/// # [`PlaneRoot::Launched`]
///
/// `plane_root_env` is exactly [`PLANE_ROOT_ON`] and nothing on this command names a workspace
/// instead. Only `-w`, and `$CHARTER_WORKSPACE` set on the command, outrank it: they name a
/// workspace for one command, which is how a root chat manages a workspace without being moved
/// into it. A blank one names nothing and outranks nothing (charter#1055). It is above the
/// tree the caller stands in, as `$CHARTER_WORKSPACE` is: a root chat that `cd`s into a
/// workspace's clone is still the chat its launcher started.
///
/// # [`PlaneRoot::Standing`] (SI-1b)
///
/// **Anywhere in the plane that is not a workspace's is the plane root** — the plane's own
/// directory, `docs/`, `.charter/`, `workspaces/` itself — when no rung that speaks for THIS
/// session answers: no `-w`, no `$CHARTER_WORKSPACE`, no workspace tree underfoot, and neither
/// the session's pointer nor its terminal's. The window files every chat standing there on the
/// plane root's tab; the ladder alone would have put it in `[workspace] default` and asked it
/// which workspace it was in.
///
/// **The plane's two defaults (`workspaces/.default`, `[workspace] default`) do not outrank
/// it**, and that is the point: they are the plane's answer for a caller that is nowhere —
/// a script run from outside the plane with `$CHARTER_ROOT` set — and they still answer
/// there, and still answer [`workspace`] asked alone. A session standing in the plane is
/// somewhere, and the pointers stay above it so `charter workspace use` keeps moving a session
/// that is not pinned to the root.
pub fn plane_root(asking: &Asking, plane_root_env: Option<&str>) -> Option<PlaneRoot> {
    if at_plane_root(asking.flag, asking.env, plane_root_env) {
        return Some(PlaneRoot::Launched);
    }
    (picked_workspace(asking).is_none() && inside_plane(asking.root, asking.cwd))
        .then_some(PlaneRoot::Standing)
}

/// [`plane_root`] for a hook or a renderer, which reads [`PLANE_ROOT_ENV`] from the
/// environment it was handed rather than from the process.
pub fn plane_root_in(asking: &Asking, env: &dyn Fn(&str) -> Option<String>) -> Option<PlaneRoot> {
    plane_root(asking, env(PLANE_ROOT_ENV).as_deref())
}

/// Where a session works: one of the plane's workspaces, or the plane root (SI-1b).
///
/// What a handoff's stamp, a handed-off chat's record of its parent and a kept report say
/// about where a chat was. **Written as one word on disk and on the wire** ([`Place::word`]):
/// a workspace's name, or [`Place::PLANE_ROOT`] — two words with a space, which no workspace
/// name can be ([`contain::workspace_name_ok`]). So every reader that held the field to the
/// name rule before this existed refuses the plane root rather than joining it onto
/// `workspaces/` — the failure that changes nothing — and every reader since reads it with
/// [`Place::read`]. A value of its own in these internal files, where every reader is
/// charter's, and never in `$CHARTER_WORKSPACE`, which programs charter did not write read
/// as a name ([`PLANE_ROOT_ENV`] says why that one is a variable of its own).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Place {
    Workspace(String),
    PlaneRoot,
}

impl Place {
    /// The plane root, as [`Place::word`] writes it.
    pub const PLANE_ROOT: &'static str = "plane root";

    /// A workspace's name, or [`Place::PLANE_ROOT`].
    pub fn word(&self) -> &str {
        match self {
            Self::Workspace(name) => name,
            Self::PlaneRoot => Self::PLANE_ROOT,
        }
    }

    /// `word` as [`Place::word`] wrote it, or `None` for anything else — a name that cannot
    /// be a workspace's is not a place.
    pub fn read(word: &str) -> Option<Self> {
        if word == Self::PLANE_ROOT {
            Some(Self::PlaneRoot)
        } else {
            contain::workspace_name_ok(word).then(|| Self::Workspace(word.to_owned()))
        }
    }

    /// The workspace, or `None` at the plane root.
    pub fn workspace(&self) -> Option<&str> {
        match self {
            Self::Workspace(name) => Some(name),
            Self::PlaneRoot => None,
        }
    }

    /// How a person reads it in a sentence: `workspace 'alpha'`, or `the plane root`.
    pub fn said(&self) -> String {
        match self {
            Self::Workspace(name) => format!("workspace '{name}'"),
            Self::PlaneRoot => "the plane root".to_owned(),
        }
    }
}

impl TryFrom<String> for Place {
    type Error = String;

    fn try_from(word: String) -> Result<Self, Self::Error> {
        Self::read(&word)
            .ok_or_else(|| format!("'{word}' is neither a workspace nor the plane root"))
    }
}

impl From<Place> for String {
    fn from(place: Place) -> Self {
        place.word().to_owned()
    }
}

/// Whether `cwd` is the plane's directory or anywhere under it — both sides resolved, for
/// [`workspace_of_tree`]'s reason.
pub fn inside_plane(root: &Path, cwd: &Path) -> bool {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let here = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    here.starts_with(root)
}

/// Whether the launcher marked this session as started at the plane root and nothing on this
/// command names a workspace instead — [`PlaneRoot::Launched`], without a directory to ask.
pub fn at_plane_root(
    flag: Option<&str>,
    workspace_env: Option<&str>,
    plane_root_env: Option<&str>,
) -> bool {
    let named = |value: Option<&str>| value.is_some_and(|v| !py_strip(v).is_empty());
    plane_root_env == Some(PLANE_ROOT_ON) && !named(flag) && !named(workspace_env)
}

/// What a command that needs a workspace says at the plane root, instead of acting on one
/// charter picked for it. A session nothing pinned there is also told how to move.
pub fn plane_root_refusal(root: &Path, by: PlaneRoot) -> String {
    let names = crate::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    let existing = if names.is_empty() {
        "none yet — `purlis workspace create <name>` makes one".to_string()
    } else {
        names
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match by {
        PlaneRoot::Launched => format!(
            "this chat was started at the plane root, so it is in no workspace and purlis \
             will not pick one for it — name the workspace with -w <workspace> (existing: \
             {existing})."
        ),
        PlaneRoot::Standing => format!(
            "this session is at the plane root and has chosen no workspace, so purlis will \
             not pick one for it — name the workspace with -w <workspace> (existing: \
             {existing}), or move this session into one with `purlis workspace use <name>`."
        ),
    }
}

/// The workspace whose directory `cwd` is inside, or `None` — `workspace.from_path`, and the
/// path arithmetic under [`crate::workspaces::Plane::workspace_of`].
///
/// **`workspaces/<ws>` itself counts** (SI-1). Python's rule was that it is the CONTAINER and
/// not a tree, so only a path with something under it counted. The app starts a workspace's
/// chats exactly there and files them under that workspace, so the two answered one question
/// differently and the chat was asked which workspace it was in. A workspace's own directory
/// holds its charter, memory and todos; standing in it is being in it. A clone
/// (`workspaces/<ws>/<repo>/…`) and a worktree at the default root
/// (`workspaces/<ws>/.worktrees/<repo>/<piece>/…`) are covered by the same walk.
///
/// **A plane that moves its worktree root with `[plane] worktrees` is not covered here**, and
/// Python's is (`worktree.locate` tries both roots). charter-app refuses a relocated root
/// outright (`worktree::relocation_refusal`), so a piece cut under one cannot exist on this
/// side to be stood in; the day that refusal lifts, this walk has to grow the second root
/// with it.
pub fn workspace_of_tree(root: &Path, cwd: &Path) -> Option<String> {
    let workspaces = root.join("workspaces");
    // Both sides resolved: on macOS a plane under `/var/folders` is reached through a link to
    // `/private/var`, and comparing the two as text answers "outside" for a path plainly
    // inside. `cwd` came off the process, which has already walked the links.
    let base = std::fs::canonicalize(&workspaces).unwrap_or(workspaces);
    let here = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let rest = here.strip_prefix(&base).ok()?;
    let mut parts = rest.components();
    let name = parts.next()?.as_os_str().to_str()?.to_string();
    contain::workspace_name_ok(&name).then_some(name)
}

/// `workspaces/.default` — the workspace `charter workspace default` nominated.
///
/// Committed, and therefore a teammate's, so it is read through the same gate every other
/// committed file charter takes a name out of goes through. NOT the "last active workspace"
/// pointer charter#124 rejected: that one is written by every `workspace use` and changes
/// under sessions that never asked; this is set once by a human and read only when every
/// other rung has missed.
pub fn declared_default_workspace(root: &Path) -> Option<String> {
    let name = committed_name(root, &root.join("workspaces").join(".default"))?;
    contain::workspace_name_ok(&name).then_some(name)
}

/// `[workspace] default` as the manifest holds it, or `None` — `instance.default_workspace_of`.
fn declared_plane_default(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(crate::names::manifest(root)).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    let named = python_str(doc.get("workspace")?.as_table()?.get("default")?)?;
    let named = py_strip(&named).to_string();
    contain::workspace_name_ok(&named).then_some(named)
}

/// A scalar TOML value as Python's `str()` renders it, or `None` for a table or an array.
///
/// A hand-edited manifest can put anything where a name goes, and charter takes `str(val)`
/// of a `str`, an `int` or a `float` and `""` of everything else. Ported because the name
/// check is what refuses these, and refusing them for a DIFFERENT reason in the two
/// implementations is how they come to disagree about the same file.
///
/// A TOML boolean is `bool` to `tomllib`, and `isinstance(True, int)` is true in Python, so
/// `default = true` reads as the name `True` there. Faithful, not endorsed.
fn python_str(value: &toml::Value) -> Option<String> {
    Some(match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Boolean(true) => "True".to_string(),
        toml::Value::Boolean(false) => "False".to_string(),
        toml::Value::Float(f) => {
            // Python's `str(float)` is the shortest round-tripping form and always carries a
            // decimal point or an exponent; Rust's `{}` drops the `.0` from an integral one.
            let rendered = format!("{f}");
            if f.is_finite() && !rendered.contains(['.', 'e', 'E']) {
                format!("{rendered}.0")
            } else {
                rendered
            }
        }
        _ => return None,
    })
}

/// `[workspace] default`, falling back to the literal `default`.
///
/// Validated, because `charter.toml` is committed: the value is joined onto `workspaces/` by
/// every command that writes, and `default = "../../esc"` in a teammate's manifest made
/// `workspace vision` read a file the plane does not contain (charter#442). A value charter
/// disagrees with degrades to the fallback rather than raising — a manifest charter cannot
/// make sense of never stops charter running.
pub fn plane_default_workspace(root: &Path) -> String {
    declared_plane_default(root).unwrap_or_else(|| BUILT_IN_WORKSPACE.to_string())
}

// --------------------------------------------------------------------------------------- //
// the persona ladder                                                                        //
// --------------------------------------------------------------------------------------- //

/// Which rung of [`persona`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonaRung {
    /// `--persona`.
    Flag,
    /// `$CHARTER_PERSONA`.
    Environment,
    /// `.charter/sessions/<sid>.persona`.
    SessionPointer,
    /// `.charter/terminals/<tid>.persona`.
    TerminalPointer,
    /// `.charter/active-persona`, written by `charter persona use`.
    ActiveFile,
    /// `[persona] default` in `charter.toml`.
    PlaneDefault,
    /// `personas/.default`, the legacy committed default.
    CommittedDefault,
    /// No rung named anything, which is a real answer: a plane may have no front door, and
    /// charter inventing one would be it choosing an identity nobody asked for.
    Nothing,
}

impl PersonaRung {
    /// How charter names this rung — `charter/persona.py:1379`'s second return value.
    pub fn label(self) -> &'static str {
        match self {
            Self::Flag => "--persona",
            Self::Environment => PERSONA_ENV_LABEL.as_str(),
            Self::SessionPointer => "session",
            Self::TerminalPointer => "terminal",
            Self::ActiveFile => "active-file",
            Self::PlaneDefault => crate::names::PLANE_MANIFEST.reads[0],
            Self::CommittedDefault => "committed-default",
            Self::Nothing => "none",
        }
    }
}

/// What the persona ladder decided: the name, or `None`, and the rung that said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePersona {
    pub name: Option<String>,
    pub rung: PersonaRung,
}

/// The active persona, by precedence — Python's `persona._resolved`, decided ONCE.
///
/// One walk and not two: `resolve_active` and `source` are two questions about a single
/// decision, and walking the rungs separately is how a session comes to adopt a persona while
/// reporting it came from nowhere.
pub fn persona(asking: &Asking) -> ActivePersona {
    let at = |name: String, rung| ActivePersona {
        name: Some(name),
        rung,
    };
    if let Some(flag) = asking.flag.filter(|f| !f.is_empty()) {
        return at(flag.to_string(), PersonaRung::Flag);
    }
    if let Some(named) = asking.env.map(py_strip).filter(|v| !v.is_empty()) {
        return at(named.to_string(), PersonaRung::Environment);
    }
    if let Some(name) = pointer(
        asking.root,
        "sessions",
        asking.ids.session.as_deref(),
        "persona",
    )
    .filter(|n| contain::persona_name_ok(n))
    {
        return at(name, PersonaRung::SessionPointer);
    }
    if let Some(name) = pointer(
        asking.root,
        "terminals",
        asking.ids.terminal.as_deref(),
        "persona",
    )
    .filter(|n| contain::persona_name_ok(n))
    {
        return at(name, PersonaRung::TerminalPointer);
    }
    if let Some(name) = local_file(asking.root, &state_dir(asking.root).join("active-persona"))
        .filter(|n| contain::persona_name_ok(n))
    {
        return at(name, PersonaRung::ActiveFile);
    }
    // The two committed rungs, and the ONLY two, name a persona that exists. A rung above
    // them naming one that does not still wins and the session has no persona — that is
    // charter's rule, and it is what makes a pointer `persona remove` left behind visible.
    if let Some(name) = declared_plane_persona(asking.root) {
        return at(name, PersonaRung::PlaneDefault);
    }
    if let Some(name) = committed_default_persona(asking.root) {
        return at(name, PersonaRung::CommittedDefault);
    }
    ActivePersona {
        name: None,
        rung: PersonaRung::Nothing,
    }
}

/// The persona this PLANE puts first — `persona.plane_default`: the declared `[persona]
/// default`, else the legacy `personas/.default`, each only when it names a persona that is
/// here. Never a session rung: this is a committed fact, the same for every session.
pub fn plane_default_persona(root: &Path) -> Option<String> {
    declared_plane_persona(root).or_else(|| committed_default_persona(root))
}

/// `[persona] default`, when it names a persona this plane defines.
///
/// A declaration naming a persona that was renamed or deleted resolves to *nothing* rather
/// than to a broken identity. Saying so out loud is `doctor`'s job; silence here is the
/// fail-toward-no-change half.
fn declared_plane_persona(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(crate::names::manifest(root)).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    let named = python_str(doc.get("persona")?.as_table()?.get("default")?)?;
    let named = py_strip(&named).to_string();
    defined_persona(root, &named)
}

/// `personas/.default` — the committed, team-wide default, when it names a persona that is
/// still there.
fn committed_default_persona(root: &Path) -> Option<String> {
    let named = committed_name(root, &root.join("personas").join(".default"))?;
    defined_persona(root, &named)
}

/// `named`, if this plane has a definition file for it — `persona.def_path(name).exists()`.
///
/// The name check comes BEFORE the existence test, and that order is charter#337: "a path
/// that exists" was never the question, and a reference climbing out of `personas/` named a
/// file the plane does not contain whose `vault:`, `role:` and `tools:` were then merged.
fn defined_persona(root: &Path, named: &str) -> Option<String> {
    if !contain::persona_name_ok(named) || named == contain::SHARED_PERSONA {
        return None;
    }
    let dir = root.join("personas").join(named).join("persona.md");
    let flat = root.join("personas").join(format!("{named}.md"));
    // The entry that is OPENED is the one gated, never `personas/` above it: a persona
    // directory that is a link out of the plane has a perfectly legal name.
    let there = |path: &Path| contain::readable(root, path).is_ok() && path.exists();
    (there(&dir) || there(&flat)).then(|| named.to_string())
}

// --------------------------------------------------------------------------------------- //
// reading one rung off disk                                                                 //
// --------------------------------------------------------------------------------------- //

/// The plane's machine-local state directory.
///
/// `$CHARTER_HOME` moves it in Python (`config._migrate_state_dir`) and is not read anywhere
/// in this binary — `cistate`, `profiletrust`, `reopen` and `hookwire` all join `.charter`
/// onto the plane. One spelling here too, rather than a second answer for these two rungs.
fn state_dir(root: &Path) -> PathBuf {
    crate::names::state(root)
}

/// The persona selections `ids` could have written: this session's pointer, this pane's, and
/// the plane-wide `.charter/active-persona` — the three files `charter persona clear` drops.
/// An id that is not one path segment names no file.
pub fn persona_pointers(root: &Path, ids: &Ids) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for (dir, id) in [
        ("sessions", ids.session.as_deref()),
        ("terminals", ids.terminal.as_deref()),
    ] {
        if let Some(id) = id.filter(|id| contain::segment_ok(id)) {
            out.push(state_dir(root).join(dir).join(format!("{id}.persona")));
        }
    }
    out.push(active_persona_file(root));
    out
}

/// The name a pointer file holds, read as the ladder reads it — `persona._read_pointer`.
pub fn read_pointer(root: &Path, path: &Path) -> Option<String> {
    local_file(root, path)
}

/// The plane-wide selection, `.charter/active-persona` — written by `charter persona use` in a
/// shell with neither a session nor a pane id.
pub fn active_persona_file(root: &Path) -> PathBuf {
    state_dir(root).join("active-persona")
}

/// How many persona selections on this machine name one persona, by rung.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pointers {
    pub sessions: usize,
    pub terminals: usize,
    pub active_file: bool,
}

/// Every selection on this machine naming `name` — `persona.pointers_naming`. Every session's
/// and every pane's, not this process's: a persona created under a removed one's name becomes
/// each of them.
pub fn pointers_naming(root: &Path, name: &str) -> Pointers {
    let count = |dir: &str| {
        std::fs::read_dir(state_dir(root).join(dir))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "persona"))
                    .filter(|p| local_file(root, p).as_deref() == Some(name))
                    .count()
            })
            .unwrap_or(0)
    };
    Pointers {
        sessions: count("sessions"),
        terminals: count("terminals"),
        active_file: local_file(root, &active_persona_file(root)).as_deref() == Some(name),
    }
}

/// A pointer file's contents, or `None` — `.charter/<dir>/<id>.<ext>`.
///
/// `id` is checked as a path SEGMENT before it is joined: it comes from the environment
/// (`$CHARTER_SESSION_ID`, `$TMUX_PANE`), the sanitiser above only removes characters charter
/// dislikes, and neither `.` nor `..` has anything removed from it.
fn pointer(root: &Path, dir: &str, id: Option<&str>, ext: &str) -> Option<String> {
    let id = id.filter(|id| contain::segment_ok(id))?;
    local_file(root, &state_dir(root).join(dir).join(format!("{id}.{ext}")))
}

/// One short line out of a file under `.charter/`, or `None`.
///
/// `contain::readable` is the WRONG gate here and would refuse every one of these: `.charter/`
/// is not one of the plane's data directories. The right one is the walk `reopen`, `hookwire`
/// and `cistate` already use for charter's own paths, asked of the FILE and not of the
/// directory above it — a link at `<sid>.workspace` redirects the read exactly as one at
/// `sessions/` does, and a check on the parent cannot see it.
///
/// **Python does not gate these two rungs at all** (`workspace._read` is a bare `read_text`),
/// so a link here is followed there and refused here. Declared in this module's header and
/// filed as charter#1146; a pointer charter wrote is never a link, so nothing honest changes.
fn local_file(root: &Path, path: &Path) -> Option<String> {
    contain::no_link_on_the_way(root, path).ok()?;
    // `false`: the walk has already refused every link on the way, so there is nothing to
    // follow and asking the kernel to would undo it.
    read_a_name(path, false)
}

/// One short line out of a COMMITTED dotfile, or `None`.
///
/// Gated by `contain::readable`, which resolves and therefore follows a link that lands back
/// inside the plane — Python's `realpath` follows it too, and a plane that links a persona
/// directory depends on that. The file is committed, so the value is a teammate's.
fn committed_name(root: &Path, path: &Path) -> Option<String> {
    contain::readable(root, path).ok()?;
    // `true`: the gate above resolves and has already decided this link lands inside the
    // plane, so what is asked here is about the file at the END of it — which is the order
    // `contain._path_refusal` keeps, and the reason it stats twice.
    read_a_name(path, true)
}

/// The stripped contents of a small regular file, or `None` for anything else.
///
/// **A `stat`, and two questions charter's `contain.file_refusal` asks with it.** Is this a
/// regular file — a FIFO committed at `workspaces/.default` would hang the read for ever and
/// a device would never end — and is it a sane size. These rungs are read on the path a
/// status line takes every turn, where a guard that has to OPEN something is a guard that can
/// block; `stat` answers both without opening anything, so there is nothing to time out.
fn read_a_name(path: &Path, follow: bool) -> Option<String> {
    let found = if follow {
        std::fs::metadata(path).ok()?
    } else {
        std::fs::symlink_metadata(path).ok()?
    };
    if !found.is_file() || found.len() > MAX_RUNG_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let name = py_strip(&text);
    (!name.is_empty()).then(|| name.to_string())
}

/// `$CHARTER_WORKSPACE`/`$CHARTER_PERSONA` is set and holds only whitespace, so the ladder
/// ignored it — charter#1055 and charter#1048.
///
/// Worth saying because the operator's export is broken and resolution going on through the
/// rungs below hides that from them. `export CHARTER_WORKSPACE=$(…)` over a command that
/// printed only a space leaves exactly this. **Empty is not reported**: it is what a launcher
/// hands every chat when the launch pinned nothing, so it would be said in every chat about
/// an export nobody wrote.
pub fn blank_in_environment(value: Option<&str>) -> bool {
    matches!(value, Some(v) if !v.is_empty() && py_strip(v).is_empty())
}

#[cfg(test)]
mod tests;
