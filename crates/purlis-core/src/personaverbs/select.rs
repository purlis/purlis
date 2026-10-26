// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter persona use <name>` — select a persona for this session and this pane. A port of
//! `commands_persona.cmd_persona_use` and `persona.set_active`.
//!
//! # What it writes, and where
//!
//! Exactly what [`crate::active`] reads back: `.charter/sessions/<sid>.persona` for this
//! session, `.charter/terminals/<tid>.persona` for this pane, and — only when the process has
//! neither id — the plane-wide `.charter/active-persona`. Each at 0600 in a 0700 directory. It
//! also records a `persona-use` event in the session's trace.
//!
//! # It touches no vault
//!
//! Selecting a persona opens nothing: the persona's vault is reached later, by `charter
//! persona secret …`, which resolves the active persona itself. So this command needed
//! nothing from the secrets registry and has no stub standing in for it.
//!
//! # A chat's launch record is not read
//!
//! Python writes no terminal pointer from inside a chat its frame launched, and says "for
//! this chat only". That record is the tmux frame's, which this charter neither reads nor
//! writes (`docs/plane-format.md`; [`crate::wscmd::select`] declares the same for
//! `workspace use`), so a chat here is treated as the terminal it runs in.
//!
//! # Inside a chat the app started, it is refused
//!
//! A chat's persona is fixed for its life (ADR 0090 as amended, #1435). [`Asking::in_chat`]
//! says the command runs in such a chat, and there it writes nothing and names the two ways
//! forward: the operator allows the vault on the chat's tab, or the chat dispatches to that
//! persona. `persona create --use` is refused there in the same words.

use std::path::Path;

use crate::active::Ids;
use crate::counts::counted;
use crate::repocmd::{Say, Sink};

/// How far a selection reaches — `set_active`'s return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Terminal,
    Session,
    Plane,
}

pub(super) fn usable(id: Option<&str>) -> Option<&str> {
    id.filter(|id| !id.is_empty() && crate::contain::segment_ok(id))
}

/// `persona.set_active`: write the pointers and return the reach of the longest-lived one.
pub fn set_active(root: &Path, name: &str, ids: &Ids) -> Scope {
    let state = crate::names::state(root);
    let _ = crate::plane::private_dir(root, &state);
    let line = format!("{name}\n");
    let sid = usable(ids.session.as_deref());
    let tid = usable(ids.terminal.as_deref());
    for (dir, id) in [("sessions", sid), ("terminals", tid)] {
        let Some(id) = id else {
            continue;
        };
        let dir = state.join(dir);
        if crate::plane::private_dir(root, &dir).is_ok() {
            let _ = crate::plane::write_private(
                root,
                &dir.join(format!("{id}.persona")),
                line.as_bytes(),
            );
        }
    }
    if sid.is_none() && tid.is_none() {
        let _ = crate::plane::write_private(root, &state.join("active-persona"), line.as_bytes());
    }
    if tid.is_some() {
        Scope::Terminal
    } else if sid.is_some() {
        Scope::Session
    } else {
        Scope::Plane
    }
}

/// `commands_persona._scope_note`: how far the selection reaches, in the words the reader
/// needs.
pub(super) fn scope_note(root: &Path, scope: Scope) -> String {
    match scope {
        Scope::Terminal => " for this terminal (kept across closing/reopening Claude)".into(),
        Scope::Session => {
            let starts = match crate::active::plane_default_persona(root) {
                Some(next) => format!("starts as '{next}'"),
                None => "starts with no persona".into(),
            };
            format!(
                " for this session only — this terminal reports no pane id, so a new session \
                 {starts}"
            )
        }
        Scope::Plane => " for this control plane (no session or pane id to scope it to)".into(),
    }
}

/// Who is asking: the ids to key the pointers on, `$CHARTER_PERSONA` as it stands, the
/// trace bucket, and the clock the trace is stamped with.
pub struct Asking<'a> {
    pub ids: &'a Ids,
    pub env_persona: Option<&'a str>,
    /// Whether this runs inside a chat the app started ([`crate::hookwire::CHAT_ENV`]), whose
    /// persona is fixed for its life (ADR 0090 as amended, #1435).
    pub in_chat: bool,
    pub bucket: &'a str,
    pub now: chrono::NaiveDateTime,
}

/// `charter persona use <name>`, and its exit code.
pub fn use_persona(root: &Path, name: &str, asking: &Asking, say: Sink) -> u8 {
    if let Some(refused) = crate::personas::name_refusal(root, name) {
        say(Say::Fail(refused.into()));
        return 1;
    }
    if asking.in_chat {
        return refuse_in_chat(name, asking.env_persona, say);
    }
    let scope = set_active(root, name, asking.ids);
    crate::trace::record(
        root,
        asking.bucket,
        "persona-use",
        &[("persona", name)],
        asking.now,
    );
    say(Say::Done(
        format!("Active persona set to '{name}'{}.", scope_note(root, scope)).into(),
    ));
    warn_env(name, asking.env_persona, say);
    say_tool_ceiling(root, name, usable(asking.ids.session.as_deref()), say);
    say_mcp_boundary(root, name, say);
    0
}

/// `charter persona clear`: drop this session's pointer, this pane's, and the plane-wide
/// file — `persona.clear_active` and `commands_persona.cmd_persona_clear`.
///
/// All three, because they are rungs of one ladder: clearing only the top one would hand
/// the shell straight back to a lower one, and "cleared" would be false the next command.
/// What the shell resolves to afterwards is READ BACK through the ladder rather than
/// predicted, so a `$CHARTER_PERSONA` that still decides, or a front door below, is named.
///
/// A chat is treated as the terminal it runs in, as [`use_persona`] treats it: this charter
/// does not read the tmux frame's launch record that told Python's apart.
pub fn clear(asking: &crate::active::Asking, say: Sink) -> u8 {
    let root = asking.root;
    let pointers = crate::active::persona_pointers(root, asking.ids);
    let held = pointers
        .iter()
        .any(|p| crate::active::read_pointer(root, p).is_some());
    for pointer in &pointers {
        if crate::contain::no_link_on_the_way(root, pointer).is_ok() {
            let _ = std::fs::remove_file(pointer);
        }
    }
    let now = crate::active::persona(asking);
    if !held {
        say(Say::Info(
            "This shell had no persona selection of its own, so nothing was cleared.".into(),
        ));
    } else if now.rung == crate::active::PersonaRung::Environment {
        say(Say::Warn(
            format!(
                "Persona selection cleared, but {} outranks every selection and still decides \
                 in this shell.",
                persona_variable()
            )
            .into(),
        ));
    } else {
        say(Say::Done("Active persona cleared.".into()));
    }
    let Some(name) = now.name.as_deref() else {
        say(Say::Info("This shell now resolves to no persona.".into()));
        return 0;
    };
    let source = now.rung.label();
    let at = format!("'{}' (via {source})", crate::personas::one_line(name));
    if super::list::exists(root, name) {
        say(Say::Info(
            format!("This shell now resolves to {at}.").into(),
        ));
    } else {
        say(Say::Info(
            format!(
                "This shell now resolves to {at}, where {}.",
                super::list::MISSING
            )
            .into(),
        ));
        say(Say::Info(
            format!("Ways out: {}.", super::list::ways_out(source)).into(),
        ));
    }
    0
}

/// The persona variable under every name it is read by (#1344).
fn persona_variable() -> String {
    crate::envvar::in_words(crate::active::PERSONA_ENV, |n| format!("${n}"), " or ")
}

/// `_warn_env`: `$CHARTER_PERSONA` outranks every pointer this wrote.
pub(super) fn warn_env(name: &str, env: Option<&str>, say: Sink) {
    let env = crate::memstore::py_strip(env.unwrap_or_default());
    if env.is_empty() || env == name {
        return;
    }
    let shown = crate::personas::one_line(env);
    say(Say::Warn(
        format!(
            "{} is set to '{shown}' and takes precedence — commands use '{shown}', not '{name}'.",
            persona_variable()
        )
        .into(),
    ));
}

/// How a chat the app started is said to run: as its persona, or with none.
fn chat_runs(env: &str) -> String {
    if env.is_empty() {
        "with no persona".into()
    } else {
        format!("as '{}'", crate::personas::one_line(env))
    }
}

/// `persona use` inside a chat the app started (#1435): a chat's persona is fixed for its
/// life (ADR 0090 as amended), so nothing is written, and the two ways to work with another
/// persona are named. The exit code is the refusal's, or 0 for a chat that names the persona
/// it already runs as, which has nothing to select.
fn refuse_in_chat(name: &str, env: Option<&str>, say: Sink) -> u8 {
    let env = crate::memstore::py_strip(env.unwrap_or_default());
    if env == name {
        say(Say::Info(
            format!(
                "This chat already runs as '{name}', and a chat's persona is fixed for its life: \
             there is nothing to select."
            )
            .into(),
        ));
        return 0;
    }
    let still = if env.is_empty() {
        "have none".to_owned()
    } else {
        let shown = crate::personas::one_line(env);
        format!("use '{shown}', with the vaults of '{shown}'")
    };
    say(Say::Fail(
        format!(
            "This chat runs {}, and a chat's persona is fixed for its life: nothing was changed, \
         and commands here still {still}.",
            chat_runs(env)
        )
        .into(),
    ));
    say(Say::Info(format!(
        "To use a vault of '{name}' here, run the command that needs it and ask the operator to \
         press Allow in the notice on this chat's tab. To have '{name}' do the work, dispatch \
         to it: `purlis dispatch --to {name}`."
    ).into()));
    1
}

/// What `persona create --use` says inside a chat the app started (#1435), where it selects
/// nothing for the same reason, before anything is made.
pub(super) fn create_use_refusal(name: &str, env: Option<&str>) -> String {
    let runs = chat_runs(crate::memstore::py_strip(env.unwrap_or_default()));
    format!(
        "This chat runs {runs}, and a chat's persona is fixed for its life, so --use cannot \
         select '{name}' here and nothing was made. Create it without --use; to have it do \
         work, dispatch to it: `purlis dispatch --to {name}`."
    )
}

/// `_say_tool_ceiling`: the tools declared since this session froze its ceiling still
/// prompt here, and the reason is said rather than left to be discovered.
fn say_tool_ceiling(root: &Path, name: &str, sid: Option<&str>, say: Sink) {
    let Some(frozen) = crate::personagate::frozen_tools(root, name, sid) else {
        return;
    };
    let added: Vec<String> = crate::personagrant::effective_tools(root, name)
        .into_iter()
        .filter(|t| !frozen.contains(t))
        .collect();
    if added.is_empty() {
        return;
    }
    say(Say::Info(
        format!(
            "  {} declared since this session started: {}.",
            counted(added.len(), "tool", "tools"),
            added.join(", ")
        )
        .into(),
    ));
    say(Say::Info(
        "  Those still prompt HERE. The tool-gate answers within the set that existed at \
         session start — `tools:` is read from a file this session can write, and freezing it \
         is what stops an edit from becoming an unprompted command. A new session picks them \
         up."
        .into(),
    ));
}

/// `_say_mcp_boundary`: a persona's MCP servers are not started for this session. They are
/// what a chat started as the persona gets ([`super::chatstart`], #1451).
fn say_mcp_boundary(root: &Path, name: &str, say: Sink) {
    let (servers, _) = super::mcp::declared(root, name);
    if servers.is_empty() {
        return;
    }
    let mut names: Vec<&String> = servers.keys().collect();
    names.sort();
    say(Say::Info(
        format!(
            "  {} declared: {}.",
            counted(names.len(), "MCP server", "MCP servers"),
            names
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .into(),
    ));
    say(Say::Info(
        "  They are NOT started for the session you are in, and servers already live here \
         (from .mcp.json or an enabled plugin) stay live. A chat started as this persona is \
         started with them, on Claude Code, once a person has approved the ones that take a \
         credential (`purlis persona approve-mcp`)."
            .into(),
    ));
}

#[cfg(test)]
mod tests;
