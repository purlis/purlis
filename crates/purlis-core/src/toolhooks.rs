// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The tool hooks other than the Bash guard — `charter/hooks.py`'s `pretooluse_read`,
//! `pretooluse_edit`, `pretooluse_dispatch` and the `posttooluse` family — and the persona
//! tool gate's place at the end of the Bash guard.
//!
//! Each handler takes the payload and the plane and returns an [`Answer`]: nothing, a line of
//! JSON for the harness, or a denial. The CLI prints it; nothing here touches stdout, so every
//! handler can be asked from a test with no process around it.
//!
//! # Exit 2 is a deliberate deny and nothing else
//!
//! A harness reads exit 2 from a `PreToolUse` hook as "block", so the only path to it is a
//! [`Answer::Deny`] whose JSON could not be written (`hooks.py:_deny`, charter#438). Every
//! other outcome — a handler with nothing to say, a payload that will not parse, a write that
//! failed — is exit 0 with nothing printed, which the harness reads as "no opinion".
//!
//! # The plane gate
//!
//! Every handler here but one is silent outside a control plane (charter#852): outside one
//! there is no workspace to nudge about, no persona to gate and no log to write. The one that
//! is not is `pretooluse-read`, the vault guard on `Read`/`Grep`, because it is the twin of the
//! Bash leak guard, which is ungated, and `$CHARTER_HOME` can put a real vault within reach of
//! a directory that holds no `charter.toml`.
//!
//! # What the Python charter did here that this leaves out, and why
//!
//! - **The trace** (`_trace`): one JSONL row per verdict, nudge and dispatch under
//!   `.charter/persona-state/trace/`. It changes no verdict and nothing in charter-app reads it;
//!   the event log (FD-9, #649) records one structured event per hook call instead.
//! - **The ask marks** (`_ask_mark_set`, `_ask_approved`): they exist only to write the
//!   `…-approved` trace row when an asked tool call goes through.
//! - **The routing ask on `pretooluse-edit`** (`_route_mark_take`): it answered a mark the
//!   persona roster set under `routing: require`, and `routing:` is retired (charter#369).
//!   Work for another persona goes to a chat of its own, by dispatch (#1434).
//! - **The dispatch log's hook rows** (`posttooluse_dispatch`, and `posttooluse-message`, which
//!   is answered as a no-op now, [`crate::hookreg::NO_OPS`]) and the in-flight ask of
//!   `pretooluse_dispatch`: each was about a persona sent out as a sub-agent, which a persona
//!   never is now (#1451). `pretooluse_dispatch` refuses that call instead.
//! - **The turn markers** (`_turn_begin`, `_turn_bump`, `_turn_end`) and `notify.plane_changed`:
//!   the tmux frame's spinner and repaint. The app has its own (`hookwire`).
//! - **`_record_reported_session`**: opencode's session report inside a tmux frame.
//! - **Committing the dispatch log** (`_commit_dispatch`): see [`crate::dispatch`].

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

use crate::hookstate::State;
use crate::toolgate::Verdict;
use crate::{leakguard, personagate, pieces, pypath};

/// What a handler decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Nothing to say: the harness carries on as it would have.
    Nothing,
    /// One line of JSON for the harness — context, an ask, or an allow.
    Say(String),
    /// A refusal. Printed as JSON; exit 2 only if that print fails.
    Deny(Verdict),
}

/// Everything a handler reads that is not a plane file.
pub struct Hook<'a> {
    /// The plane root when there is one, else the process's directory — `config.ROOT`.
    pub root: &'a Path,
    /// `charter.toml` is at `root` — `config.HAS_CONTROL_PLANE`, the MARKER and not a resolve.
    pub in_plane: bool,
    /// The harness's payload; `Null` when it would not parse.
    pub payload: &'a Value,
    /// The environment, as a lookup.
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// The process's directory — the ladders' cwd rung.
    pub cwd: &'a Path,
    pub now: DateTime<Utc>,
    /// This machine's name ([`crate::dispatch::host`]): a label, and the name its logs are filed
    /// under only until a device id is minted ([`Hook::log_name`]).
    pub host: &'a str,
}

impl Hook<'_> {
    /// The name this device's logs are filed under: its device id, read from the machine store
    /// the environment names and never minted here (FD-25, [`crate::machine::log_name`]).
    fn log_name(&self) -> String {
        crate::machine::log_name(
            crate::machine::config_root_in(self.env).as_deref(),
            self.host,
        )
    }

    /// `data.get(key) or ""` for a string field.
    fn text(&self, key: &str) -> &str {
        self.payload
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    fn tool_name(&self) -> &str {
        self.text("tool_name")
    }

    fn tool_input(&self) -> &Value {
        self.payload.get("tool_input").unwrap_or(&Value::Null)
    }

    /// `(tool_input.get(key) or "").strip()` for a string field.
    fn input_text(&self, key: &str) -> &str {
        crate::memstore::py_strip(
            self.tool_input()
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
    }

    /// The payload's `session_id`, as it came.
    fn session_id(&self) -> Option<&str> {
        self.payload.get("session_id").and_then(Value::as_str)
    }

    fn state(&self) -> State {
        State::of(self.root)
    }

    /// The active persona, by the ladder — `persona.resolve_active()`.
    pub fn persona(&self) -> Option<String> {
        let named = (self.env)(crate::active::PERSONA_ENV);
        crate::active::persona(&crate::active::Asking {
            root: self.root,
            cwd: self.cwd,
            flag: None,
            ids: &crate::active::Ids::of(self.env),
            env: named.as_deref(),
        })
        .name
    }

    /// `_workspace_session`: the chat's id inside the app, the payload's outside it.
    fn workspace_session(&self) -> Option<String> {
        (self.env)(crate::active::SESSION_ID_ENV)
            .filter(|v| !v.is_empty())
            .or_else(|| self.session_id().map(str::to_owned))
    }

    /// `workspace.resolve(session_id=…)`.
    fn workspace(&self, session: Option<&str>) -> String {
        let mut ids = crate::active::Ids::of(self.env);
        ids.session = crate::hookstate::session(session, self.env);
        let pinned = (self.env)(crate::active::WORKSPACE_ENV);
        crate::active::workspace(&crate::active::Asking {
            root: self.root,
            cwd: self.cwd,
            flag: None,
            ids: &ids,
            env: pinned.as_deref(),
        })
        .name
    }

    /// Whether this session is at the plane root, in no workspace
    /// ([`crate::active::plane_root`]): the app started it there (SI-1), or it stands in the
    /// plane outside every workspace and has chosen none (SI-1b).
    fn at_plane_root(&self, session: Option<&str>) -> bool {
        let mut ids = crate::active::Ids::of(self.env);
        ids.session = crate::hookstate::session(session, self.env);
        let pinned = (self.env)(crate::active::WORKSPACE_ENV);
        crate::active::plane_root_in(
            &crate::active::Asking {
                root: self.root,
                cwd: self.cwd,
                flag: None,
                ids: &ids,
                env: pinned.as_deref(),
            },
            self.env,
        )
        .is_some()
    }

    /// `_touch_piece`: the worker in the payload's `cwd` is alive.
    pub fn touch_piece(&self) {
        let cwd = self.text("cwd");
        if !self.in_plane || cwd.is_empty() {
            return;
        }
        let persona = self.persona();
        pieces::touch(
            self.root,
            Path::new(cwd),
            self.session_id(),
            persona.as_deref(),
            self.now,
        );
    }

    /// `_mark_guard_seen`: the Bash guard ran, under this chat's harness, so `purlis doctor`'s
    /// `guard seen` row has a date to give ([`crate::guardseen`]). Best-effort.
    pub fn mark_guard_seen(&self) {
        if !self.in_plane {
            return;
        }
        let harness = crate::names::ENV_PREFIX
            .spellings()
            .find_map(|prefix| (self.env)(&format!("{prefix}HARNESS")))
            .filter(|h| !h.is_empty());
        let source = crate::guardseen::Source::of(self.env);
        // The Claude Code config folder this chat runs under: `$CLAUDE_CONFIG_DIR` as set, or
        // the home's `.claude`.
        let folder = match (self.env)("CLAUDE_CONFIG_DIR") {
            Some(dir) => Some(PathBuf::from(dir)),
            None => (self.env)("HOME").map(|home| Path::new(&home).join(".claude")),
        };
        let _ = crate::guardseen::mark(
            &self.state(),
            harness.as_deref(),
            source,
            folder.as_deref(),
            self.now,
        );
    }
}

/// Where each file tool carries its path — `_PATH_KEYS`. Both spellings, because the key is
/// the HARNESS's: Claude Code's `file_path`, opencode's `filePath`.
pub const PATH_KEYS: [&str; 5] = [
    "file_path",
    "path",
    "notebook_path",
    "filePath",
    "notebookPath",
];

/// Python's truth of a JSON value.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
    }
}

/// `[str(ti[k]) for k in _PATH_KEYS if ti.get(k)]`.
fn targets(input: &Value) -> Vec<String> {
    PATH_KEYS
        .iter()
        .filter_map(|k| {
            let v = input.get(*k);
            truthy(v).then(|| crate::pyrepr::str_json(v.unwrap_or(&Value::Null)))
        })
        .collect()
}

/// A `PreToolUse` refusal with charter's words around it.
fn deny(reason: String) -> Answer {
    Answer::Deny(Verdict {
        reason: reason.chars().take(70).collect(),
        shape: None,
        denial: reason,
    })
}

/// `hookSpecificOutput` for `event`, as `json.dumps` writes it.
fn say(event: &str, fields: &[(&str, String)]) -> Answer {
    let mut inner = serde_json::Map::new();
    inner.insert("hookEventName".into(), Value::String(event.into()));
    for (k, v) in fields {
        inner.insert((*k).into(), Value::String(v.clone()));
    }
    let mut outer = serde_json::Map::new();
    outer.insert("hookSpecificOutput".into(), Value::Object(inner));
    Answer::Say(crate::pyjson::dumps(
        &Value::Object(outer),
        None,
        ", ",
        ": ",
    ))
}

// ---- PreToolUse ---------------------------------------------------------------------------

/// `pretooluse_read`: refuse a `Read`/`Grep` that would print a vault's plaintext into the
/// transcript. **Not plane-gated** — see the module header.
pub fn pretooluse_read(hook: &Hook) -> Answer {
    hook.touch_piece();
    let tool = hook.tool_name();
    if tool != "Read" && tool != "Grep" {
        return Answer::Nothing;
    }
    let input = hook.tool_input();
    let named = targets(input);
    // `_names_a_vault_path` and nothing else — the Bash route's predicate, with no private
    // half of the answer here (#462).
    if named.iter().any(|t| leakguard::names_a_vault_path(t)) {
        return deny(leakguard::READ_REASON.to_string());
    }
    // `Grep` walks; `Read` opens one file. A `Grep` with no path searches the directory it
    // stands in, which is the commonest spelling of the search that read every vault (#474).
    if tool != "Grep" {
        return Answer::Nothing;
    }
    let state_dir = crate::plane::state_dir(hook.root);
    let operands = if named.is_empty() {
        vec![".".to_string()]
    } else {
        named
    };
    let Some(walked) =
        leakguard::walk_into_guarded_state(hook.text("cwd"), &operands, &[], &state_dir)
    else {
        return Answer::Nothing;
    };
    // `Grep` has no exclude; its own narrowing stands in for one, answered by looking.
    let glob = input
        .get("glob")
        .filter(|g| truthy(Some(g)))
        .map(crate::pyrepr::str_json)
        .unwrap_or_default();
    if !glob.is_empty() && !leakguard::glob_selects_inside(&walked, &glob, 512) {
        return Answer::Nothing;
    }
    let name = walked
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    deny(format!(
        "walks a directory tree that contains the plane's own `{name}` — every file in it \
         would be printed into the transcript, and none of them is named on this call. {}",
        leakguard::walk_fix_for(&walked)
    ))
}

/// `_state_write_reason`'s sentence.
pub const STATE_WRITE_REASON: &str = "writes purlis's own state directly (that directory \
     decides which commands run without a prompt). Use the purlis command that owns it — \
     `purlis vault add`, `purlis secret set`; a persona is chosen in the app's new-chat \
     picker";

/// `pretooluse_edit`: refuse a `Write`/`Edit` into charter's own state directory, which holds
/// the tool-gate ceiling, the persona pointers and the vault registry.
pub fn pretooluse_edit(hook: &Hook) -> Answer {
    if !hook.in_plane {
        return Answer::Nothing;
    }
    let named = targets(hook.tool_input());
    if named.is_empty() {
        return Answer::Nothing;
    }
    let cwd = match hook.text("cwd") {
        "" => hook.cwd.to_string_lossy().into_owned(),
        given => given.to_string(),
    };
    let state = pypath::realpath(&crate::plane::state_dir(hook.root).to_string_lossy());
    for t in named {
        let joined = if pypath::is_abs(&t) {
            t
        } else {
            pypath::join(&cwd, &t)
        };
        let p = pypath::realpath(&joined);
        if p == state || p.starts_with(&format!("{state}/")) {
            return deny(STATE_WRITE_REASON.to_string());
        }
    }
    Answer::Nothing
}

/// **`pretooluse_edit`, where a chat's edit of purlis's own configuration is a proposed
/// change** (#1975, PC-2): in a sandboxed Claude Code chat, a `Write`, `Edit` or `MultiEdit` of
/// a proposable file ([`crate::proposed::proposable`]) is worked out against the file as it is
/// now and handed to the app by `hand`, and the tool is refused with
/// [`crate::proposed::PROPOSED_SAID`]: the harness never writes the file, and the person applies
/// it, in the window. Where the change cannot be made as the tool would make it, the tool is
/// refused with why. Where the app does not take it (`hand` answers `false`: no app, an older
/// one, or one that refused it), or where policy turns proposed changes off (PC-14), the edit is
/// [`pretooluse_edit`]'s, and the chat's own sandbox refuses it as before.
///
/// **What `hand` is given is only a proposal**: the app reads the file itself to show it and
/// checks the base again at Apply, so nothing here is trusted to say what the file holds.
pub fn pretooluse_edit_proposing(
    hook: &Hook,
    hand: &dyn Fn(crate::hookwire::ProposeAsk) -> bool,
) -> Answer {
    proposing(hook, hand).unwrap_or_else(|| pretooluse_edit(hook))
}

/// The answer to a proposable edit, or none where the edit is not one.
fn proposing(hook: &Hook, hand: &dyn Fn(crate::hookwire::ProposeAsk) -> bool) -> Option<Answer> {
    if !hook.in_plane || !crate::sandbox::chat_is_sandboxed_in(hook.env) {
        return None;
    }
    let harness = (hook.env)(crate::hookwire::HARNESS_ENV);
    if harness
        .as_deref()
        .and_then(crate::sandbox::harness_of_registry)
        != Some(crate::harness::Harness::ClaudeCode)
    {
        return None;
    }
    let chat: u32 = (hook.env)(crate::hookwire::CHAT_ENV)?.parse().ok()?;
    if crate::sandbox::policy::Locks::of(hook.root)
        .proposed_changes_refused()
        .is_some()
    {
        return None;
    }
    let cwd = match hook.text("cwd") {
        "" => hook.cwd.to_path_buf(),
        given => PathBuf::from(given),
    };
    match crate::proposed::from_edit(hook.root, hook.payload, &cwd)? {
        Err(why) => Some(deny(why)),
        Ok(proposed) => hand(crate::hookwire::ProposeAsk {
            chat,
            path: proposed.path.display().to_string(),
            base: proposed.base,
            change: proposed.change,
        })
        .then(|| deny(crate::proposed::PROPOSED_SAID.to_owned())),
    }
}

/// `pretooluse_dispatch`: refuse a sub-agent call whose type is a persona, and name the
/// dispatch route (#1451). A persona runs as its own chat, with its own vault and hosts; a
/// sub-agent runs inside this chat's process and has this chat's.
///
/// A call with no type, or a type that is no persona here (a harness's own helper, a
/// hand-written agent), is not this hook's to judge: it runs, as this chat's persona. The name
/// is asked of the plane's personas as they are now, drafts included, and a hand-written agent
/// that shares a persona's name is refused with it: the name is the persona's.
///
/// And refuse purlis's own `dispatch` tools to a sub-agent ([`crate::dispatchguard`]).
pub fn pretooluse_dispatch(hook: &Hook) -> Answer {
    if !hook.in_plane {
        return Answer::Nothing;
    }
    let tool = hook.tool_name();
    // purlis's own dispatch tools, called from inside a sub-agent (#1436): refused here, where
    // the payload says who made the call. The chat's own call is the app's to decide.
    let agent_id = hook.text("agent_id");
    let harness = (hook.env)(crate::hookwire::HARNESS_ENV);
    let caller = crate::handoffguard::Caller {
        agent_id: (!agent_id.is_empty()).then_some(agent_id),
        harness: harness.as_deref(),
        permission_mode: None,
    };
    if let Some(why) = crate::dispatchguard::tool_refusal(tool, caller) {
        return deny(why);
    }
    if tool != "Task" && tool != "Agent" {
        return Answer::Nothing;
    }
    let agent = hook.input_text("subagent_type");
    if agent.is_empty() {
        return Answer::Nothing;
    }
    if crate::personaverbs::names(hook.root)
        .iter()
        .any(|persona| persona == agent)
    {
        // A draft runs no chat, so its refusal does not name the dispatch route (F4).
        let draft = crate::personaverbs::is_draft(hook.root, agent);
        return deny(crate::personaverbs::retired::subagent_refusal(agent, draft));
    }
    Answer::Nothing
}

/// The persona tool gate at the end of the Bash guard: `allow` when the active persona
/// declares the program, `None` to leave the harness's prompt alone. Plane-gated.
pub fn persona_allow(hook: &Hook) -> Option<String> {
    if !hook.in_plane {
        return None;
    }
    let command = hook
        .tool_input()
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let persona = hook.persona();
    let session = crate::hookstate::session(hook.session_id(), hook.env);
    let cwd = match hook.text("cwd") {
        "" => hook.cwd.to_string_lossy().into_owned(),
        given => given.to_string(),
    };
    let approved = || {
        let config = crate::machine::config_root_in(hook.env)?;
        let store = crate::machine::read(&config).store;
        let persona = persona.as_deref()?;
        crate::machine::spellings_of(hook.root)
            .iter()
            .find_map(|plane| store.approved_grant(plane, persona).map(str::to_owned))
    };
    let (name, binary) = personagate::decide(&personagate::Ask {
        plane: hook.root,
        persona: persona.as_deref(),
        session: session.as_deref(),
        command,
        cwd: &cwd,
        approved: &approved,
    })?;
    Some(personagate::allowed(&name, &binary))
}

/// A Bash command that RECORDS a memory — `_MEM_RECORD_RE`. It resets the cadence nudge,
/// because a memory written through the CLI is invisible to `PostToolUse` on a file.
fn mem_record() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b(?:workspace|persona)[\s\x{1C}-\x{1F}]+(?:remember|note)\b")
            .expect("compiles")
    })
}

/// The Bash guard's bookkeeping: the heartbeat, and the cadence reset a memory-recording
/// command earns. Plane-gated, like all of `pretooluse`'s writers.
pub fn pretooluse_bookkeeping(hook: &Hook) {
    if !hook.in_plane {
        return;
    }
    hook.touch_piece();
    hook.mark_guard_seen();
    let command = hook
        .tool_input()
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if mem_record().is_match(command) {
        memnudge_set(hook, 0);
    }
}

// ---- PostToolUse --------------------------------------------------------------------------

/// Re-surface the record-memory habit every this many memory-less file changes —
/// `_MEM_NUDGE_EVERY`.
pub const MEM_NUDGE_EVERY: u64 = 12;

fn mem_path() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"/(?:personas/[^/]+|workspaces/[^/]+)/(?:memory|refs)/").expect("compiles")
    })
}

fn ws_clone() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"/workspaces/([^/]+)/([^/]+)/").expect("compiles"))
}

/// `<sessions>/<sid>.memnudge`, for an id that is one path segment.
fn memnudge_file(hook: &Hook) -> Option<PathBuf> {
    let sid = hook.session_id().filter(|s| !s.is_empty())?;
    crate::contain::segment_ok(sid).then(|| hook.state().sessions().join(format!("{sid}.memnudge")))
}

fn memnudge_set(hook: &Hook, n: u64) {
    if let Some(file) = memnudge_file(hook) {
        let _ = hook.state().write(&file, n.to_string().as_bytes());
    }
}

/// `_memnudge_bump`: one more file change since the last recorded memory; the new count, or 0
/// with no session to count for.
fn memnudge_bump(hook: &Hook) -> u64 {
    let Some(file) = memnudge_file(hook) else {
        return 0;
    };
    let n = hook
        .state()
        .read_text(&file)
        .and_then(|t| crate::memstore::py_strip(&t).parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    let _ = hook.state().write(&file, n.to_string().as_bytes());
    n
}

/// `_ws_edit_first_this_session`: true the FIRST time a clone in `ws` is edited this session,
/// marking it so the next is not.
fn ws_edit_first(hook: &Hook, ws: &str) -> bool {
    let Some(sid) = hook.session_id().filter(|s| !s.is_empty()) else {
        return true;
    };
    let state = hook.state();
    let key = crate::hookstate::safe(&format!("{sid}-{ws}"));
    let marker = state.dir().join("ws-edit-nudge").join(key);
    if marker.exists() {
        return false;
    }
    let _ = state.write(&marker, b"1");
    true
}

/// `memory_share_note`: what recording a memory will actually do on this plane — which is
/// whatever the plane's next save does with it, by `[plane] mode` (ADR 0051). Nothing commits a
/// memory on its own. A request mode's request is named as the forge the plane's origin is on names it.
pub fn memory_share_note(root: &Path) -> String {
    use crate::planesave::Mode;
    let request = || crate::forge::request_words_of(root, root).request_noun();
    match crate::planesave::Settings::read(root).plane.mode.value {
        None => "It stays on THIS MACHINE until the plane is saved — `purlis save` commits and \
                 pushes it."
            .to_owned(),
        Some(Mode::Off) => "It stays on THIS MACHINE — this plane's `[plane] mode` is `off`, so \
                            purlis commits nothing; commit and push it yourself if the team \
                            needs it."
            .to_owned(),
        Some(Mode::Commit) => "It is committed with the plane's next save, but NOT pushed — this \
                               plane's `[plane] mode` is `commit`."
            .to_owned(),
        Some(Mode::Push) => {
            "It reaches the team with the plane's next save — `purlis save` pushes it.".to_owned()
        }
        Some(Mode::Pr) => {
            let request = request();
            format!(
                "It reaches the team once a person merges the plane's {request} — this plane's \
                 `[plane] mode` is `pr`, so the next save pushes it to this machine's save \
                 branch and opens or updates that {request}."
            )
        }
        Some(Mode::PrMerge) => {
            let request = request();
            format!(
                "It reaches the team once the plane's {request} merges by itself — this plane's \
                 `[plane] mode` is `pr-merge`, so the next save pushes it to this machine's save \
                 branch, opens or updates that {request}, and sets it to merge when its checks \
                 pass."
            )
        }
    }
}

/// `_mem_cadence_nudge`.
fn mem_cadence_nudge(hook: &Hook, count: u64) -> String {
    let session = hook.workspace_session();
    let ws = hook.workspace(session.as_deref());
    // A plane-root chat is in no workspace, so no workspace's memory is where its facts go.
    let live = !hook.at_plane_root(session.as_deref())
        && crate::workspaces::Plane::open(hook.root).is_live(&ws);
    let how = if live {
        format!("`purlis workspace remember \"<fact>\"` (workspace **{ws}**)")
    } else if let Some(active) = hook.persona() {
        format!("`purlis persona remember {active} \"<fact>\"`")
    } else {
        "`purlis workspace remember \"<fact>\"` (make the workspace LIVE to share) or `purlis \
         persona remember <p> \"<fact>\"`"
            .to_string()
    };
    format!(
        "⬢ Memory check — ~{count} file changes since your last recorded memory. Recording \
         durable memory is a standing part of the flow, and it fades on long sessions. If this \
         work produced something durable — a decision, a gotcha, a verified fact, a *why* — \
         record it now so it survives this session: {how}. {} If nothing here is worth keeping, \
         carry on — don't record filler.",
        memory_share_note(hook.root)
    )
}

/// `posttooluse` on `Write`/`Edit`/`MultiEdit`: warn when a memory or ref just written looks
/// like it holds a secret; otherwise count the change toward the record-memory habit, and on
/// the first edit in a LIVE workspace's clone say what the workspace flow expects.
pub fn posttooluse(hook: &Hook) -> Answer {
    if !hook.in_plane {
        return Answer::Nothing;
    }
    hook.touch_piece();
    if !matches!(hook.tool_name(), "Write" | "Edit" | "MultiEdit") {
        return Answer::Nothing;
    }
    let input = hook.tool_input();
    let fp = [input.get("file_path"), input.get("filePath")]
        .into_iter()
        .flatten()
        .find(|v| truthy(Some(v)))
        .map(crate::pyrepr::str_json)
        .unwrap_or_default();
    if fp.is_empty() {
        return Answer::Nothing;
    }
    if let Some(answer) = theme_draft_said(hook, &fp) {
        return answer;
    }
    let norm = format!("/{}", fp.replace('\\', "/")).replace("//", "/");
    if mem_path().is_match(&norm) {
        memnudge_set(hook, 0);
        return secret_scan(hook, input, &fp);
    }
    let count = memnudge_bump(hook);
    if let Some(m) = ws_clone().captures(&norm)
        && !matches!(&m[2], "memory" | "refs")
    {
        let (ws, repo) = (&m[1], &m[2]);
        if crate::workspaces::Plane::open(hook.root).is_live(ws) && ws_edit_first(hook, ws) {
            return say(
                "PostToolUse",
                &[(
                    "additionalContext",
                    format!(
                        "⬢ You're changing **{repo}** in workspace **{ws}**. Per the workspace \
                         flow, record a **workspace memory** (what changed + why + the repo \
                         commit) before you finish — `purlis workspace remember \"<…>\"` (one \
                         file per memory under `workspaces/{ws}/memory/`, recall with `purlis \
                         workspace recall`). And keep the **purlis** current \
                         (`workspaces/{ws}/workspace.md`): if this work shifts the goal, adds a \
                         key decision, or introduces a new term, update its Vision / Context / \
                         Glossary so a teammate or a fork inherits the real picture. Both are \
                         committed + shared + auto-saved. Commit the actual code inside the repo \
                         (its own remote), then `purlis workspace snapshot` to record the \
                         branch (in a sandboxed chat, purlis's app records it for you). Do this \
                         **without asking the engineer** — it's the flow."
                    ),
                )],
            );
        }
    }
    if count > 0 && count.is_multiple_of(MEM_NUDGE_EVERY) {
        return say(
            "PostToolUse",
            &[("additionalContext", mem_cadence_nudge(hook, count))],
        );
    }
    Answer::Nothing
}

/// **A write into a chat's theme draft** (#2332, `crate::theme::drafts`): the engine reads the
/// draft as the window draws it, and the chat is told when the window does not draw it: the hard
/// floors it breaks, or why the engine refused it. A draft the window draws says nothing, and so
/// does any draft but the chat's own. The
/// draft is read by the engine through handles refusing a link, under its caps; the path named
/// only says which draft, never what is read.
fn theme_draft_said(hook: &Hook, fp: &str) -> Option<Answer> {
    use crate::theme::drafts;
    let path = if Path::new(fp).is_absolute() {
        PathBuf::from(fp)
    } else {
        hook.cwd.join(fp)
    };
    let root = crate::plane::state_dir(hook.root).join(drafts::DRAFTS);
    let real = |at: &Path| std::fs::canonicalize(at).unwrap_or_else(|_| at.to_path_buf());
    // As written, or as the disk spells it (a temporary folder is reached through a link on
    // macOS): the file itself may not be there yet, so its folder is resolved.
    let below = match path.strip_prefix(&root) {
        Ok(below) => below.to_path_buf(),
        Err(_) => {
            let whole = path.parent().map(real)?.join(path.file_name()?);
            whole.strip_prefix(real(&root)).ok()?.to_path_buf()
        }
    };
    let mut parts = below.components().map(|part| part.as_os_str().to_str());
    let (Some(Some(chat)), Some(Some(folder)), Some(_)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if !drafts::is_chat_id(chat) || !crate::theme::is_folder_name(folder) {
        return None;
    }
    // Only the chat's own draft is spoken of: the chat its harness says it is, never another's.
    if (hook.env)(crate::hookwire::CHAT_ID_ENV).as_deref() != Some(chat) {
        return None;
    }
    let started_in = (hook.env)(crate::sandboxblock::CHAT_DIR_ENV).map(PathBuf::from);
    let workspace =
        crate::active::workspace_of_tree(hook.root, started_in.as_deref().unwrap_or(hook.cwd));
    let config = crate::machine::config_root_in(hook.env);
    let place = drafts::Place {
        plane: hook.root,
        workspace: workspace.as_deref(),
    };
    let folders = crate::theme::Folders::of(config.as_deref(), Some(hook.root), place.workspace);
    let draft = drafts::Draft {
        chat: chat.to_owned(),
        folder: folder.to_owned(),
        kept_to: drafts::kept_to(&folders, place.workspace, folder),
    };
    let read = drafts::read(config.as_deref(), place, &draft, true);
    let drafted = drafts::drafted(
        read,
        None,
        None,
        crate::theme::Appearance::Dark,
        crate::theme::Contrast::Standard,
    );
    const MOST: usize = 8;
    let said = if !drafted.broken.is_empty() {
        let each: Vec<String> = drafted
            .broken
            .iter()
            .take(MOST)
            .map(drafts::BrokenIn::said)
            .collect();
        format!(
            "purlis does not draw the theme draft {folder}: it breaks a hard floor, so the window \
             keeps its last good version. {}. Fix it and save again; `purlis theme check \
             draft:{folder}` says the same.",
            each.join("; ")
        )
    } else if !drafted.refused.is_empty() {
        let each: Vec<&str> = drafted
            .refused
            .iter()
            .take(MOST)
            .map(|one| one.said.as_str())
            .collect();
        format!(
            "purlis does not draw the theme draft {folder}: {}. Fix it and save again; `purlis \
             theme check draft:{folder}` says the same.",
            crate::shown::readable(&each.join("; "), 1200)
        )
    } else {
        return None;
    };
    Some(say("PostToolUse", &[("additionalContext", said)]))
}

/// `_posttooluse_secret_scan`: what was written, and the file as it now stands, checked for a
/// credential's SHAPE. Named by kind, never by the matched text.
///
/// Python's question first, of what was written as its repr and the file joined. Then each of
/// them on its own as the project save reads the file (#1315): as written and through its
/// escapes, the file parsed when its name says JSON or TOML
/// ([`crate::secretshape::kind_as_read`]) — so this warning, the first word on a memory, says
/// what the save that publishes it will refuse.
fn secret_scan(hook: &Hook, input: &Value, fp: &str) -> Answer {
    let written: Vec<&Value> = ["content", "new_string", "new_str"]
        .iter()
        .filter_map(|k| input.get(*k).filter(|v| truthy(Some(v))))
        .collect();
    let mut text = ["content", "new_string", "new_str"]
        .iter()
        .map(|k| {
            input
                .get(*k)
                .filter(|v| truthy(Some(v)))
                .map(crate::pyrepr::str_json)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    // The file is read where the PROCESS stands, as `Path(fp).read_text()` reads it.
    let on_disk = if Path::new(fp).is_absolute() {
        PathBuf::from(fp)
    } else {
        hook.cwd.join(fp)
    };
    let now = crate::memstore::read_text(&on_disk);
    if let Some(now) = &now {
        text.push('\n');
        text.push_str(now);
    }
    let form = crate::secretshape::Structured::of(fp);
    let as_read = || {
        written
            .iter()
            .filter_map(|v| v.as_str())
            .chain(now.as_deref())
            .find_map(|one| crate::secretshape::kind_as_read(form, one))
    };
    let Some(kind) = crate::secretshape::secret_kind(&text).or_else(as_read) else {
        return Answer::Nothing;
    };
    let name = Path::new(fp)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    say(
        "PostToolUse",
        &[(
            "additionalContext",
            format!(
                "⚠ SECURITY: the memory/ref you just wrote ({name}) appears to contain a secret \
                 ({kind}). Persona AND workspace memory/refs are committed and shared — secrets \
                 must NEVER go there. Remove it now and store the value in the vault instead \
                 (`purlis persona secret set <key>` / `purlis vault`)."
            ),
        )],
    )
}

/// `posttooluse_skill`: nothing to the harness, and the worker in the payload's `cwd` is alive.
/// The run a Skill call starts is [`skill_started`]'s, which every tool hook asks.
pub fn posttooluse_skill(hook: &Hook) -> Answer {
    hook.touch_piece();
    Answer::Nothing
}

/// **The skill run a finished tool call starts, where it starts one** (SK-3, #2766), read by the
/// chat's harness's adapter from the call ([`crate::harness::HarnessAdapter::skill_start`]): the
/// skill its Skill tool names, or, for a chat briefed on its skills, the skill whose `SKILL.md`
/// a read names ([`crate::skills::readable_by_a_briefed_chat`]). The run's id is minted here,
/// and the host opens the run under it, once per turn ([`crate::skillspan`]). **Where no host
/// listens** (a terminal's own harness), the hook logs the run's start itself, under the active
/// persona, so no use of a skill goes unrecorded; its end is not, as nothing counts its signals.
/// A chat that names no harness is Claude Code's: a terminal's own `claude` with purlis's
/// plugin.
pub fn skill_started(hook: &Hook) -> Option<Box<crate::hookwire::SkillStarted>> {
    use crate::harness::{Harness, SkillStart};
    use purlis_skills::runs::StartedBy;
    let kind = crate::envvar::lookup(crate::hookwire::HARNESS_ENV, hook.env);
    let adapter = Harness::of_chat(kind.as_deref())?.adapter();
    // A prompt the person sent started a skill (SK-12): only the start of a skill Use skill…
    // typed, which the host holds and alone opens; logged nowhere where no host listens.
    if let Some(skill) = adapter.prompt_started(hook.payload) {
        return Some(Box::new(skill_run(hook, skill, StartedBy::UseSkill)));
    }
    let start = adapter.skill_start(hook.tool_name(), hook.tool_input())?;
    let (skill, started_by) = match start {
        SkillStart::Named(skill) => (skill, StartedBy::SkillTool),
        SkillStart::Reading(read) => {
            let skills = crate::skills::readable_by_a_briefed_chat(hook.env);
            let cwd = hook
                .payload
                .get("cwd")
                .and_then(Value::as_str)
                .map_or(hook.cwd, Path::new);
            let skill = purlis_skills::runs::started_by_a_read(&read, cwd, &skills)?;
            (skill.name.clone(), StartedBy::Read)
        }
    };
    let started = skill_run(hook, skill, started_by);
    if crate::envvar::lookup(crate::hookwire::SOCKET_ENV, hook.env).is_none() {
        started_unheard(hook, &started);
    }
    Some(Box::new(started))
}

/// **The skill run a start of `skill` in `hook`'s chat opens**, by `started_by`: its id minted
/// here, the workspace the chat works in (none at the plane root) and which copy of the skill it
/// is ([`crate::skills::whose`]). The one maker of a run's start, for a hook and for purlis's MCP
/// server's `skill_load` alike ([`crate::skilltools::start_run`]).
pub fn skill_run(
    hook: &Hook,
    skill: String,
    started_by: purlis_skills::runs::StartedBy,
) -> crate::hookwire::SkillStarted {
    let session = hook.workspace_session();
    let workspace =
        (!hook.at_plane_root(session.as_deref())).then(|| hook.workspace(session.as_deref()));
    let whose = if hook.in_plane {
        crate::skills::whose(
            hook.root,
            workspace.as_deref(),
            hook.persona().as_deref(),
            &skill,
        )
    } else {
        purlis_skills::runs::Whose::default()
    };
    crate::hookwire::SkillStarted {
        skill,
        run: crate::reopen::mint(),
        started_by,
        workspace: whose.workspace,
        level: whose.level,
    }
}

/// **A run's start no host heard**: a terminal's own harness, or a host its line could not
/// reach. Logged in the plane's skill log under the active persona, so no use of a skill goes
/// unrecorded; its end is not, as nothing counts its signals. Nothing outside a plane.
pub fn started_unheard(hook: &Hook, started: &crate::hookwire::SkillStarted) {
    if !hook.in_plane {
        return;
    }
    let persona = hook.persona();
    let kind = crate::envvar::lookup(crate::hookwire::HARNESS_ENV, hook.env);
    let harness = crate::harness::Harness::of_chat(kind.as_deref());
    let starting = crate::skilluse::Starting {
        harness: harness.map(crate::harness::Harness::name),
        ..started.into()
    };
    let _ = crate::skilluse::started(
        hook.root,
        &starting,
        persona.as_deref(),
        hook.now,
        &hook.log_name(),
    );
}

/// `posttooluse_dispatch`: nothing to the harness, since #1451. It logged which persona a
/// `Task`/`Agent` call was sent to; a persona is no longer a sub-agent, so there is no such
/// call to log, and a dispatch to a persona is recorded by the app that starts its chat. The
/// word stays wired because the app hears it: a helper came back, which ends "waiting on a
/// helper" in the chat's line of what it is doing ([`crate::doing`]).
pub fn posttooluse_dispatch(_hook: &Hook) -> Answer {
    Answer::Nothing
}

#[cfg(test)]
mod tests;
