// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter clone <repo>...`: clone on demand into a workspace. Python's `cmd_clone`.
//!
//! # The destination is a name a forge chose
//!
//! A clone lands at `workspaces/<ws>/<name>`, and `<name>` comes out of `inventory/repos.json`
//! — a tracked file, filled from a forge's API and editable by anybody with a commit. So the
//! exact path created is gated, not its parent: the name must be a repo name
//! ([`contain::repo_name_ok`] — no `..`, no separator, no leading `-` git could read as an
//! option, no leading `.`), the path must stay inside the workspace without passing through a
//! link ([`confine::within_workspace`]), and must land in the plane's data directories
//! ([`contain::writable`]). The path handed to git is the one checked, after `--`.
//!
//! # The URL is one charter built
//!
//! HTTPS from the record's `web_url`, or the record's `ssh_url` rewritten through its forge's
//! own SSH forms — never a string handed through (Python #335: `ext::sh -c …` is a transport
//! that runs a command). On top of Python's rule: a URL carrying a user or a token before
//! its host is refused without being repeated, and a host the plane does not manage is
//! refused, so a tracked file cannot send a clone somewhere the forge's credential helper
//! would then be asked about.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::Value;

use super::{Say, Sink, banner, submodules};
use crate::cloning::Stage;
use crate::contain;
use crate::counts::counted;
use crate::forge::{self, Forge, py_str};
use crate::guest::{self, Hidden, Status};
use crate::inventory;
use crate::manifest::Ownership;
use crate::pyrepr::repr_str;
use crate::repos;
use crate::tui::Markup;
use crate::workspaces::Plane;
use crate::worktree::confine::{self, Outside};
use crate::worktree::git;

mod landing;

/// How many clones run at once. Python's `CLONE_WORKERS`.
pub const WORKERS: usize = 8;

/// What `clone` needs besides the repo names.
pub struct Request<'a> {
    pub root: &'a Path,
    pub ws: &'a str,
    pub repos: &'a [String],
    /// When the manifest says it was last touched: `updated_at`.
    pub now: chrono::DateTime<chrono::Utc>,
    /// Who it says touched it: `updated_by`, `$USER` in Python.
    pub author: &'a str,
    /// The hosts a clone may reach, where it runs for a sandboxed chat (#1335): the chat's
    /// sandbox egress ([`crate::sandbox::hosts`]). A record whose URL names another host is
    /// refused before git runs. `None` is no such limit: the terminal's own command.
    pub hosts: Option<&'a [String]>,
    /// The window watching this clone (#4079): `None` for the terminal's own command.
    pub watch: Option<&'a Watch<'a>>,
}

/// A window watching a clone: what it is told as the clone runs, the person's Cancel, and what
/// it is told afterwards that no line of the clone's says.
pub struct Watch<'a> {
    heard: &'a (dyn Fn(Stage, u8) + Sync),
    called_off: &'a AtomicBool,
    stopped: AtomicBool,
    sign_in: Mutex<Option<String>>,
    #[cfg(test)]
    stand_in: landing::StandIn,
}

impl<'a> Watch<'a> {
    /// `heard` hears each move of a clone's stage and its percent of the whole clone, which
    /// never goes back. Raising `called_off` stops git and leaves nothing behind.
    pub fn new(heard: &'a (dyn Fn(Stage, u8) + Sync), called_off: &'a AtomicBool) -> Self {
        Self {
            heard,
            called_off,
            stopped: AtomicBool::new(false),
            sign_in: Mutex::new(None),
            #[cfg(test)]
            stand_in: landing::StandIn::default(),
        }
    }

    /// Whether the person's Cancel stopped a clone before it landed.
    pub fn was_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// The server a clone needs the person to sign in to, where the forge refused them.
    pub fn sign_in_to(&self) -> Option<String> {
        self.sign_in.lock().ok().and_then(|held| held.clone())
    }
}

/// What happened to one repo.
enum Outcome {
    /// Not attempted, for the reason given.
    Refused(String),
    /// Something is already at the destination.
    Exists(PathBuf),
    /// Cloned, onto this branch.
    Cloned {
        dest: PathBuf,
        forge: Forge,
        branch: String,
    },
    /// The person stopped it, and nothing of it is left.
    Stopped,
    /// git tried and failed: purlis's sentence, and then git's own words.
    Failed { head: String, said: String },
}

/// Run `clone`. Returns the exit status.
pub fn clone(request: &Request, say: Sink) -> u8 {
    let root = request.root;
    let cfg = forge::load_config(root).unwrap_or_default();
    let group = forge::group_of(&cfg, 0);
    let doc = match inventory::load(root, &group) {
        Ok(doc) => doc,
        Err(why) => {
            say(Say::Fail(why.into()));
            return 1;
        }
    };
    let all = inventory::repos(root, &doc, &forge::exclude_of(&cfg, 0));
    if all.is_empty() {
        say(Say::Fail(
            "Nothing to clone: this plane has no inventory, and its own repo could not be \
             derived (no origin, or a forge it does not declare)."
                .into(),
        ));
        say(Say::Info("  Build one: purlis discover".into()));
        return 1;
    }

    let mut targets: Vec<Value> = Vec::new();
    for name in request.repos {
        match inventory::find(&all, name) {
            Some(record) => targets.push(record.clone()),
            None => say(Say::Warn(
                format!(
                    "Unknown repo (not in inventory): {name} — check the name (`purlis status`) or \
                 `purlis discover` if it's new to the group."
                )
                .into(),
            )),
        }
    }
    if targets.is_empty() {
        say(Say::Fail(
            "No matching repos. Give one or more repo names/paths from the inventory (see \
             them: `purlis status`; refresh from GitLab: `purlis discover`)."
                .into(),
        ));
        return 1;
    }

    let ws = request.ws;
    banner(ws, say);
    let ws_dir = match workspace_dir(request, say) {
        Ok(dir) => dir,
        Err(why) => {
            say(Say::Fail(why.into()));
            return 1;
        }
    };

    // The checkouts already in the workspace get the layer FIRST, and silently: Python's
    // `ensure` scaffolds the workspace before cloning, and the scaffold wires what is there
    // without a word. The announcement after the clone is then only about what this command
    // brought in — a clone somebody made by hand last week is not news.
    wire(root, ws, &mut |_| {});

    if targets.len() > 1 {
        say(Say::Info(
            format!(
                "Cloning {} into '{ws}', {} at a time …",
                counted(targets.len(), "repo", "repos"),
                WORKERS.min(targets.len())
            )
            .into(),
        ));
    }
    let outcomes = clone_all(request, &ws_dir, &targets);

    // Printed here, from one thread, in the order the repos were asked for: eight workers
    // printing as they finish interleave into something nobody can scan for which failed.
    let mut failures = 0;
    let mut members: Vec<String> = Vec::new();
    let window = forge::gitcredential::in_the_window();
    for (record, outcome) in targets.iter().zip(outcomes) {
        let name = py_str(record.get("name").unwrap_or(&Value::Null));
        let dest = match outcome {
            Outcome::Refused(why) => {
                failures += 1;
                let quoted = match record.get("name") {
                    Some(Value::String(s)) => repr_str(s),
                    Some(other) => py_str(other),
                    None => "None".into(),
                };
                say(Say::Fail(format!("{quoted}: not cloned — {why}.").into()));
                continue;
            }
            Outcome::Exists(dest) => {
                say(Say::Info(
                    format!("{name}: already cloned in '{ws}'").into(),
                ));
                dest
            }
            Outcome::Cloned {
                dest,
                forge,
                branch,
            } => {
                let shown = if branch.is_empty() {
                    name.clone()
                } else {
                    format!("{name} ({branch})")
                };
                let rel = dest.strip_prefix(root).unwrap_or(&dest);
                say(Say::Done(
                    format!(
                        "{name} → {} ({shown} via {}, HTTPS)",
                        rel.display(),
                        if window {
                            forge.kind.display()
                        } else {
                            forge.kind.cli()
                        }
                    )
                    .into(),
                ));
                dest
            }
            Outcome::Stopped => {
                say(Say::Info(
                    format!("{name}: stopped, and nothing kept").into(),
                ));
                continue;
            }
            Outcome::Failed { head, said } if said.is_empty() => {
                failures += 1;
                say(Say::Fail(head.into()));
                continue;
            }
            Outcome::Failed { head, said } => {
                failures += 1;
                // What git said is its own lines, each escaped: a line end in it starts a line of
                // git's, never one that reads as purlis's.
                say(Say::Fail(Markup::lines([
                    head.into(),
                    Markup::lines_of(&said),
                ])));
                continue;
            }
        };
        // Reported BEFORE the docs hint: an empty submodule directory is what breaks the next
        // command, and the clone itself still succeeded.
        submodules::report(root, &dest, &name, None, None, say);
        // Made by the app for a chat, git ran no filter of your config's (#1413): a checkout
        // whose .gitattributes asks for one, Git LFS above all, holds what is stored.
        if git::isolation().is_some() {
            let at = dest
                .strip_prefix(root)
                .unwrap_or(&dest)
                .display()
                .to_string();
            for note in super::unread::filter_notes(&dest, &name, &at) {
                say(Say::Warn(note.into()));
            }
        }
        hint_docs(&dest, &name, say);
        if let Some(dir) = dest.file_name() {
            members.push(dir.to_string_lossy().into_owned());
        }
    }
    wire(root, ws, say);
    record_membership(request, &members, say);
    u8::from(failures > 0)
}

/// The workspace directory, made and scaffolded first when it is not there (#1382).
///
/// Python's `ensure` created a workspace a clone named, and [`crate::wscmd::ensure`] is that
/// step here: the directory AND its baseline, so the new workspace is not one `reinit` then
/// flags on every turn. Only a workspace that is ABSENT is made: anything else that stops
/// [`confine::workspace_dir`] — a bad name, a link at `workspaces/<ws>`, a path outside the
/// project — is refused as it was, before anything is written, so nothing is scaffolded
/// through a link.
///
/// **Not for a sandboxed chat.** Its clone is run by the app on its behalf
/// ([`Request::hosts`] is `Some`), and that route clones; making a workspace is a separate
/// step, `purlis workspace create`, which the person or the chat takes on its own.
fn workspace_dir(request: &Request, say: Sink) -> Result<PathBuf, String> {
    let (root, ws) = (request.root, request.ws);
    if !contain::workspace_name_ok(ws) {
        return Err(format!(
            "invalid workspace name '{ws}' (use letters, digits, '.', '_', '-'; must not start \
             with a dot)"
        ));
    }
    match confine::workspace_dir(root, ws) {
        Ok(dir) => Ok(dir),
        Err(Outside::NoWorkspace { .. }) if request.hosts.is_some() => Err(format!(
            "no workspace '{ws}' — a clone the app runs for a sandboxed chat clones into a \
             workspace that exists and makes none. Create it first (`purlis workspace create \
             {ws}`)."
        )),
        Err(Outside::NoWorkspace { .. }) => {
            crate::wscmd::ensure::ensure(root, ws, request.now, request.author)?;
            let dir = confine::workspace_dir(root, ws).map_err(|why| why.to_string())?;
            say(Say::Info(
                format!(
                    "Made workspace '{ws}' to clone into → workspaces/{ws}/ (LOCAL — private; \
                 `purlis workspace live` to share)"
                )
                .into(),
            ));
            Ok(dir)
        }
        Err(other) => Err(other.to_string()),
    }
}

/// Clone every target, [`WORKERS`] at a time, answering in the order asked.
fn clone_all(request: &Request, ws_dir: &Path, targets: &[Value]) -> Vec<Outcome> {
    let slots: Mutex<Vec<Option<Outcome>>> = Mutex::new((0..targets.len()).map(|_| None).collect());
    let next = AtomicUsize::new(0);
    // A brokered clone's isolation is held per thread, so each worker takes it on (#1335), and
    // the window's own clone its credential scope (#4079): with neither, a worker's git would be
    // the terminal's.
    let isolation = git::isolation();
    let window = forge::gitcredential::held();
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(targets.len()) {
            scope.spawn(|| {
                forge::gitcredential::within(window.as_ref(), || {
                    git::within(isolation.as_ref(), || {
                        loop {
                            let i = next.fetch_add(1, Ordering::SeqCst);
                            let Some(record) = targets.get(i) else {
                                break;
                            };
                            let outcome = clone_one(request, ws_dir, record);
                            if let Ok(mut slots) = slots.lock() {
                                slots[i] = Some(outcome);
                            }
                        }
                    });
                });
            });
        }
    });
    slots
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|o| o.unwrap_or_else(|| Outcome::Refused("purlis lost track of this clone".into())))
        .collect()
}

/// Python's `contain.refusal`: the sentence a name that is a path is refused with.
pub fn not_a_segment(name: &str) -> String {
    format!(
        "'{}' is not a name — it is a path. This is read from a committed file and joined onto \
         a directory, so it may name one entry there and nothing else: no '/', no '\\', no '.' \
         or '..', and nothing absolute",
        crate::shown::escaped(name)
    )
}

/// The one destination a record may be cloned to, gated as itself.
pub fn destination(root: &Path, ws: &str, ws_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !contain::segment_ok(name) {
        return Err(not_a_segment(name));
    }
    if !contain::repo_name_ok(name) {
        return Err(format!(
            "'{}' is not a name purlis will take for a repo: a clone is named by letters, \
             digits, '.', '_' and '-', starting with a letter or a digit, so that no clone can \
             be read as a git option or hidden as a dot-directory",
            crate::shown::escaped(name)
        ));
    }
    // A workspace's skills live in its `skills/` and `skills.local/` (ADR 0099), which a chat
    // is denied writing (SK-9): a clone of that name would be a repo no chat could work in.
    if [purlis_skills::PROJECT_DIR, purlis_skills::LOCAL_DIR]
        .iter()
        .any(|taken| taken.eq_ignore_ascii_case(name))
    {
        return Err(format!(
            "purlis will not clone '{}' under that name: a workspace keeps its skills in a              folder of that name. Clone it under another name",
            crate::shown::escaped(name)
        ));
    }
    // A clone's directory is named in `workspace.json` and that manifest is committed, so the
    // name travels (charter-app#96). The alphabet above admits `nul` and `alpha.`, and a
    // forge may honestly hold a repo called either.
    if let Err(why) = contain::mintable(name) {
        return Err(format!(
            "purlis will not clone '{}' under that name: {why}. Clone it by hand under a \
             name that travels, and purlis will pick the checkout up from there",
            crate::shown::escaped(name)
        ));
    }
    let dest =
        confine::within_workspace(root, ws, &ws_dir.join(name)).map_err(|e| e.to_string())?;
    contain::writable(root, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// The HTTPS URL a record is cloned from, or why there is none. Python's `_https_url`, and
/// then the two refusals it lacks.
pub fn https_url(record: &Value, root: &Path) -> Result<String, String> {
    let text = |key: &str| {
        record
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let web = text("web_url");
    let web = web.trim_end_matches('/');
    let url = if web.starts_with("https://") {
        format!("{web}.git")
    } else {
        let ssh = text("ssh_url");
        let (base, forms) = Forge::for_record(record).insteadof();
        match forms.iter().find(|p| ssh.starts_with(p.as_str())) {
            Some(prefix) => format!("{base}{}", &ssh[prefix.len()..]),
            None => {
                return Err(
                    "its inventory record carries no HTTPS clone URL, and the `ssh_url` \
                            it does carry is not a form this forge recognises — purlis will \
                            not hand git a string it did not build"
                        .into(),
                );
            }
        }
    };
    let after_scheme = &url["https://".len()..];
    let authority = after_scheme.split('/').next().unwrap_or_default();
    // Not repeated back: what sits before an `@` is a user, a token or both, and a refusal is
    // a line in a terminal and a transcript.
    if authority.contains('@') {
        return Err(
            "its URL carries a user or a credential before the host, and purlis will not put \
             one on a command line or into a clone's config — the forge's own CLI holds the \
             credential"
                .into(),
        );
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("its URL holds whitespace or a control character".into());
    }
    // By the server first: a forge declared on a port is that port's (#1864).
    if forge::resolve_host(&url, root).is_none() {
        return Err(format!(
            "its URL names host '{}', which is neither a forge's default host nor one this \
             plane declares in charter.toml — a tracked inventory could otherwise send a clone \
             anywhere",
            crate::shown::escaped(&forge::host_of(&url))
        ));
    }
    Ok(url)
}

/// Clone ONE repo. Prints nothing: [`clone`] renders every outcome afterwards, in order.
fn clone_one(request: &Request, ws_dir: &Path, record: &Value) -> Outcome {
    let (root, ws) = (request.root, request.ws);
    let name = record
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let dest = match destination(root, ws, ws_dir, name) {
        Ok(dest) => dest,
        Err(why) => return Outcome::Refused(why),
    };
    match std::fs::symlink_metadata(&dest) {
        Ok(found) if found.file_type().is_symlink() => {
            return Outcome::Refused(format!(
                "workspaces/{ws}/{name} is a symlink, and purlis will not clone through one"
            ));
        }
        Ok(_) if git::isolation().is_some() || !landing::unfinished(&dest) => {
            return Outcome::Exists(dest);
        }
        _ => {}
    }
    let url = match https_url(record, root) {
        Ok(url) => url,
        Err(why) => return Outcome::Refused(why),
    };
    let forge = Forge::for_clone(record, &url);
    if let Some(why) = request
        .hosts
        .and_then(|hosts| unreachable_host(hosts, &url))
    {
        return Outcome::Refused(why);
    }
    let failed = if git::isolation().is_some() {
        brokered(ws_dir, name, &dest, &url, &forge)
    } else {
        let route = landing::Route {
            root,
            ws,
            ws_dir,
            name,
            dest: &dest,
            url: &url,
            forge: &forge,
            watch: request.watch,
        };
        match landing::land(&route) {
            landing::Landed::Cloned => None,
            landing::Landed::Refused(why) => return Outcome::Refused(why),
            landing::Landed::Stopped => return Outcome::Stopped,
            landing::Landed::Failed { head, said } => Some(Outcome::Failed { head, said }),
        }
    };
    if let Some(failed) = failed {
        return failed;
    }
    // The remote the window cloned from is the one its saves push to (#752).
    forge::gitcredential::record_push_remote(&dest, &url);
    crate::gitpolicy::apply(&dest, root);
    let branch = git::run(
        &dest,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        git::READ,
    )
    .ok()
    .filter(|r| r.ok())
    .map(|r| r.line().to_string())
    .unwrap_or_default();
    Outcome::Cloned {
        dest,
        forge,
        branch,
    }
}

/// A clone the app runs for a sandboxed chat: to `dest` itself, under [`git::NETWORK`]'s deadline,
/// which `gitask` bounds its wait on. What failed, or `None`.
fn brokered(ws_dir: &Path, name: &str, dest: &Path, url: &str, forge: &Forge) -> Option<Outcome> {
    let cli = forge.kind.cli();
    let head = format!(
        "{name}: clone failed — no access, network, or {cli} isn't authed (`{cli} auth status`). \
         Skipping."
    );
    let dest_arg = dest.display().to_string();
    // No `--branch`: the remote's HEAD is its real default branch, where the inventory's
    // field may be one `discover` assumed, or one the forge has since moved off.
    let cloned = git::run_network(
        ws_dir,
        Some(&forge::helper_for(forge)),
        &["clone", "--", url, &dest_arg],
    );
    let run = match cloned {
        Ok(run) => run,
        Err(why) => {
            return Some(Outcome::Failed {
                head,
                said: why.to_string(),
            });
        }
    };
    if run.ok() {
        return None;
    }
    let mut said = failure(&run, dest);
    // A clone the app made for a chat read none of your git config (#1413): where it could
    // not reach its host and yours would have changed its route, say which key (#1550).
    if (run.code.is_none() || super::unread::reads_as_a_route_failure(&run.err))
        && let Some(note) = super::unread::network_note(&super::unread::global_entries(), url)
    {
        said = format!("{said}\n{note}");
    }
    Some(Outcome::Failed { head, said })
}

/// Why `url`'s host is not one `hosts` lets a sandboxed chat reach, or `None` when it is
/// (#1335, D-5). Checked by the app before git runs, so a clone it makes for a chat reaches no
/// host the chat itself could not.
fn unreachable_host(hosts: &[String], url: &str) -> Option<String> {
    let host = forge::host_of(url);
    (!crate::sandbox::egress::allows(hosts, &host)).then(|| {
        format!(
            "its host '{}' is not one this chat's sandbox may reach, so the app did not clone \
             it for the chat. The project's `[sandbox] egress` in {} says which hosts a chat \
             reaches (the `forge` preset holds the forges')",
            crate::shown::escaped(&host),
            crate::plane::MANIFEST
        )
    })
}

/// What a failed clone says: git's own words, and why when the why is charter's rule.
fn failure(run: &git::Run, dest: &Path) -> String {
    let said = run.err.trim().to_string();
    match run.code {
        None => format!(
            "git did not finish within {} seconds and was stopped; whatever it left at {} is \
             yours to inspect or remove — purlis does not delete it",
            git::NETWORK.as_secs(),
            dest.display()
        ),
        Some(_) => explained(said),
    }
}

/// git's words, with why when the why is charter's rule.
fn explained(said: String) -> String {
    if said.contains("transport 'ssh' not allowed") {
        format!(
            "{said}\ngit was told to use SSH for this URL — a `url.<base>.insteadOf` in your \
             git config rewrites it — and purlis clones over HTTPS with the forge CLI's token \
             only (docs/git-policy.md)"
        )
    } else {
        said
    }
}

/// `  ↳ <name> ships its own <file>` for the first of the three a clone has.
fn hint_docs(dest: &Path, name: &str, say: Sink) {
    for file in ["CLAUDE.md", "AGENTS.md", "README.md"] {
        if dest.join(file).exists() {
            say(Say::Info(
                format!("  ↳ {name} ships its own {file} — read it before working there.").into(),
            ));
            return;
        }
    }
}

/// Give every clone in the workspace the plane's layer, and say so once per clone that got
/// something. Python's `_wire_clones` for the clones — a workspace's linked worktrees get
/// theirs where they are cut.
fn wire(root: &Path, ws: &str, say: Sink) {
    let Ok(found) = repos::clones(root, ws) else {
        return;
    };
    for repo in found.repos {
        let wired = guest::wire(root, &repo.path);
        let label = &repo.name;
        if let Hidden::Blocked(..) = wired.hidden {
            say(Say::Warn(
                format!("{label}: {}", wired.refusal(&repo.path)).into(),
            ));
            continue;
        }
        if wired.rows.iter().any(|r| r.status == Status::Blocked) {
            say(Say::Warn(
                format!("{label}: {}", wired.refusal(&repo.path)).into(),
            ));
            continue;
        }
        let wrote = wired
            .rows
            .iter()
            .filter(|r| matches!(r.status, Status::Created | Status::Refreshed))
            .count();
        if wrote > 0 {
            // The files, and the exclude block that hides them: Python counts both.
            say(Say::Info(
                format!(
                    "{label}: purlis's layer written ({}) and hidden in that repo's \
                 .git/info/exclude — `git status` there is unaffected, and nothing purlis wrote \
                 can be committed.",
                    counted(wrote + 1, "file", "files")
                )
                .into(),
            ));
        }
    }
}

/// Record what is now in the workspace in its committed manifest. Python's
/// `_record_membership` and `record_members`: additive, sorted, and never a branch.
fn record_membership(request: &Request, members: &[String], say: Sink) {
    if members.is_empty() {
        return;
    }
    let Ok(ws) = Plane::open(request.root).workspace(request.ws) else {
        return;
    };
    let now = request.now.format("%Y-%m-%dT%H:%M:%S+00:00").to_string();
    // From the first look to the last write, one lock (#1249 U2): a removal or another clone
    // writing in between would lose its change to the manifest read here.
    let _held = match ws.manifest_lock() {
        Ok(held) => held,
        Err(why) => {
            say(Say::Warn(
                format!("{why}, so it does not record what was just cloned.").into(),
            ));
            return;
        }
    };
    let (_, owner) = ws.manifest();
    if owner == Ownership::Absent {
        let rows: Vec<Value> = repos::clones(request.root, request.ws)
            .map(|c| c.repos)
            .unwrap_or_default()
            .into_iter()
            .map(|r| serde_json::json!({"name": r.name}))
            .collect();
        let fresh = serde_json::json!({
            "name": request.ws,
            "description": "",
            "repos": rows,
            "updated_at": now,
            "updated_by": request.author,
        });
        let _ = ws.write_manifest(&fresh);
    }
    let (doc, owner) = ws.manifest();
    match owner {
        Ownership::Charter => {}
        Ownership::Operator => {
            say(Say::Warn(
                format!(
                    "workspaces/{0}/workspace.json was not written by purlis — left untouched, so \
                 it does not record what was just cloned. `purlis workspace snapshot {0}` \
                 rewrites it deliberately.",
                    request.ws
                )
                .into(),
            ));
            return;
        }
        Ownership::Absent => return,
    }
    let Some(mut doc) = doc else {
        return;
    };
    let rows: Vec<Value> = doc
        .get("repos")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|r| r.is_object() && r.get("name").is_some_and(forge::truthy))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let have: Vec<String> = rows
        .iter()
        .map(|r| py_str(r.get("name").unwrap_or(&Value::Null)))
        .collect();
    let mut add: Vec<String> = Vec::new();
    for name in members {
        if !name.is_empty() && !have.contains(name) && !add.contains(name) {
            add.push(name.clone());
        }
    }
    if add.is_empty() {
        return;
    }
    let mut all = rows;
    all.extend(add.into_iter().map(|n| serde_json::json!({"name": n})));
    all.sort_by_key(|r| py_str(r.get("name").unwrap_or(&Value::Null)));
    if let Some(map) = doc.as_object_mut() {
        map.insert("repos".into(), Value::Array(all));
        map.insert("updated_at".into(), Value::String(now));
        map.insert(
            "updated_by".into(),
            Value::String(request.author.to_string()),
        );
    }
    // A manifest charter could not write is Python's "blocked", which it does not announce.
    let _ = ws.write_manifest(&doc);
}

#[cfg(test)]
mod tests;
