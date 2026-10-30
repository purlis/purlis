// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter init` and `charter reinit`: scaffold a control plane, and heal one a newer
//! charter expects more of.
//!
//! A port of `charter/commands.py:cmd_init` and `cmd_reinit`, and of what they call.
//!
//! # The contract, and where it is stricter than the Python
//!
//! **Additive and idempotent: create only what is absent, never modify an existing value.**
//! When a path charter wants is occupied by something it cannot safely touch, it names the
//! blocker, deletes and renames nothing, still creates everything else, and exits 1.
//!
//! `init` writes into a directory an operator POINTS it at — possibly a repository with
//! content they care about, possibly one somebody else committed. So every path either
//! command writes or reads is gated first, and **a path that resolves outside the plane is
//! a blocker**, whatever put it there: a committed `.gitignore -> ~/.bashrc`, a `personas`
//! link to another tree, a dangling `charter.toml -> /elsewhere/x`. Nothing is read or
//! written through it, and the gate is `contain::within_plane` on the exact path, never its
//! parent. A link that lands back INSIDE the plane is followed, as Python follows it.
//! A link that lands in the plane's own `.git` is refused too: nothing charter writes here
//! belongs in a repository's internals.
//!
//! Where the Python leaves a file in a way the contract does not allow, this does not follow
//! it, and each place says so: a CRLF `.gitignore` or `charter.toml` keeps its line endings
//! here (Python's universal newlines rewrite them), and a settings file Python would crash
//! on is left alone and reported.
//!
//! # The one place this deliberately disagrees with the Python charter
//!
//! **`charter init` at the top of an existing git repository writes nothing and says how to
//! ask** ([`repo_is_not_a_plane_yet`]). Python scaffolds the plane into that repository and
//! then offers the first clone as a printed command; charter-app reverses which of those two
//! is the default, because ADR 0033 made "which plane" a directory picked in a file dialog
//! and a directory picked from a list has nobody standing in it. ADR 0035 decided it and
//! charter-app spec decision 27 records it.
//!
//! It is the FIRST declared hole in spec decision 15's byte-for-byte guarantee, and Python is
//! frozen (decision 17), so it does not follow. The recorded scenario carries it with a
//! `divergence` note, and the replay holds the app to its side of it (ADR 0046).
//!
//! # What `init` does not do here, on purpose
//!
//! - **It installs no software.** Python's `init` runs `claude plugin install` when `claude`
//!   is on `PATH` (`_provision_harnesses`). The app installs nothing anywhere: it ships its
//!   own plugin and loads it into each chat it starts, for that session alone
//!   (`crate::plugin`).
//! - **It writes nothing outside the plane.** Python's `init` puts opencode's plugin, command
//!   and instructions into `~/.config/opencode` (`OpenCodeHarness.wire`). The app loads its
//!   own opencode shim into each opencode chat it starts, for that session alone
//!   (`crate::opencode`), and `charter plugin install` is the one command that writes the
//!   guard into `~/.config/opencode` for a terminal chat (ADR 0058). The plane's own
//!   `opencode.json` ask rule IS written: it is part of the plane.
//!
//! # The other half of ADR 0035's default: adopting the repo
//!
//! The refusal above is only half of what ADR 0035 decided. Its sentence is *"`charter init`
//! on an existing repo **adopts that repo as the plane's first clone** and makes the plane
//! beside it"*, and until charter-app#175 charter could do the refusing and not the adopting:
//! `--clone-this-repo` answered with a refusal of its own, on the grounds that the clone is
//! handed charter's git policy and `gitpolicy.apply` was not ported. **It has been since M2.5
//! (#64)** — `crates/purlis-core/src/gitpolicy.rs` is the whole module, `apply` included —
//! so that note was stale, and the refusal it justified outlived it.
//!
//! So [`InitArgs::adopt`] names the repository to adopt, `firstclone` does the cloning for
//! both shapes, and `--clone-this-repo` clones rather than refusing. **Where the plane goes is
//! still typed, never guessed.** ADR 0035 says the plane is made "beside" the repo; it does
//! not say charter picks that directory, and `init` writing `../<name>-plane` out of a
//! directory it was pointed at would break this module's own contract — *"It writes nothing
//! outside the plane"* — for the sake of saving one `mkdir`. The operator names the plane's
//! directory and `--adopt` names the repo, which is the same two answers the app's dialog asks
//! for (charter-app#175, step 4).

pub(crate) mod firstclone;
pub mod fromremote;
pub mod planefile;
pub mod settings;
pub mod text;

use std::path::{Path, PathBuf};

use crate::counts::counted;
use crate::names;
use crate::plane::Place;
use crate::tui::Markup;
use settings::Wrote;

/// One line a command says, in the voice `charter/util.py` gives it.
///
/// Each holds [`Markup`]: a name or a value read at run time enters it escaped (SK-54).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Say {
    /// `• ` — a note.
    Info(Markup),
    /// `✓ ` — it worked.
    Ok(Markup),
    /// `! ` — something the operator should look at.
    Warn(Markup),
    /// `✗ ` — something asked for did not happen.
    Err(Markup),
}

impl Say {
    /// The line with the mark `charter init` prints in front of it, which is how a window
    /// shows it too: an operator reading `init` in the app reads what a terminal would have
    /// shown them.
    pub fn marked(&self) -> String {
        match self {
            Say::Info(text) => format!("• {text}"),
            Say::Ok(text) => format!("✓ {text}"),
            Say::Warn(text) => format!("! {text}"),
            Say::Err(text) => format!("✗ {text}"),
        }
    }

    /// Every line, [`Say::marked`], one per line: a refusal shown **verbatim, and all of it**.
    pub fn in_full(lines: &[Say]) -> String {
        lines.iter().map(Say::marked).collect::<Vec<_>>().join("\n")
    }
}

/// Everything a command said, in order, and its exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub said: Vec<Say>,
    pub code: u8,
    /// Why the forge could not be read, when `code` is [`ASKS_FOR_FORGE`]: what a caller that
    /// asks in its own words (the app's two buttons) says, without parsing `said`.
    pub asks_forge: Option<String>,
}

/// The forges `init --forge` takes (`forge/registry.py:KINDS`), sorted as argparse lists them.
pub const FORGES: [&str; 2] = ["github", "gitlab"];

/// The directories every plane has (`instance.BASELINE_DIRS`).
pub const BASELINE_DIRS: [&str; 3] = ["personas", "inventory", "workspaces"];

/// The exit status of an `init` that cannot tell which forge the project's repos are on and
/// asks for it (#839). It wrote nothing; the caller asks the operator and runs it again.
pub const ASKS_FOR_FORGE: u8 = 2;

/// What `charter init` was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitArgs {
    /// The forge the project's repos are on, or `None` to read it from the remote of the repo
    /// the project is made for ([`fromremote`]). Asked for when that remote does not say.
    pub forge: Option<String>,
    pub owner: String,
    pub host: Option<String>,
    pub clone_this_repo: bool,
    /// Make the git repository this is run in BE the plane — the old default, now asked for
    /// by name (ADR 0035, spec decision 27). It decides nothing outside a repository's top
    /// level, where there is no repository to colonise.
    pub plane_is_this_repo: bool,
    /// **ADR 0035's default, as the thing charter does rather than the thing it describes**:
    /// a repository SOMEWHERE ELSE, adopted as this plane's first clone (charter-app#175).
    ///
    /// The plane is scaffolded at the directory `init` was pointed at, exactly as it is with
    /// no flag at all, and then this repo is cloned into the plane's first workspace and left
    /// otherwise untouched. `--clone-this-repo` is the same act with the source fixed to the
    /// plane root, so the two are refused together rather than silently ranked.
    pub adopt: Option<PathBuf>,
    /// When the first workspace's manifest records it was made — `None` is the wall clock.
    /// Only a run that adopts or clones writes one, which is why it is an option rather than
    /// a field every caller has to have an answer for.
    pub now: Option<chrono::DateTime<chrono::Utc>>,
    /// The front-door persona to scaffold, or `None` for `--no-front-door`.
    pub front_door: Option<String>,
}

impl InitArgs {
    /// What the app scaffolds a plane with — the New project dialog's Advanced form and the
    /// first run's local project (FR-4) — given the one decision and the one repo the dialog
    /// asks for.
    ///
    /// `forge` is the operator's answer when an earlier run asked for it
    /// ([`ASKS_FOR_FORGE`]); `None` reads it from the repo's remote, as `charter init` does.
    /// An answer about a self-managed remote keeps that remote's host, and the owner its path
    /// names, so neither is typed again (#881, [`fromremote::hosted_repo`]).
    pub fn for_the_app(
        plane_is_this_repo: bool,
        adopt: Option<PathBuf>,
        forge: Option<crate::forge::Kind>,
    ) -> Self {
        let hosted = forge
            .zip(adopt.as_deref())
            .and_then(|(kind, repo)| fromremote::hosted_repo(repo, kind));
        let (owner, host) = hosted.map_or((String::new(), None), |hosted| {
            (hosted.owner, Some(hosted.host))
        });
        Self {
            // `charter init`'s rule (#839): the forge, and the owner with it, come from the
            // repo's remote, or are asked for.
            forge: forge.map(|kind| kind.word().to_owned()),
            owner,
            host,
            // Never from the app: it is `adopt` with the source fixed to the plane's own
            // directory, and the dialog asks for the source as a directory of its own.
            clone_this_repo: false,
            plane_is_this_repo,
            adopt,
            // The wall clock: a window has no `--now`, and the first workspace's manifest is
            // stamped with when the operator actually made it.
            now: None,
            front_door: Some("steward".to_owned()),
        }
    }
}

/// The `.gitignore` a fresh plane gets (`commands._GITIGNORE_BASELINE`).
pub const GITIGNORE_BASELINE: &str = "\
# Per-task workspaces (workspaces/<name>/). LOCAL by default = fully private (clones,
# memory, manifest all ignored). Made LIVE via `charter workspace live <name>` un-ignores
# its workspace.json + memory/ in a managed block here (see charter/workspace.py).
/workspaces/*/*
!/workspaces/.gitkeep

# Per-developer secret vaults + registry (plaintext secrets, tokens, file paths).
# NEVER commit this — it holds credentials.
/.charter/
/.purlis/

# This machine's own harness permissions. Its committed
# sibling `.claude/settings.json` is deliberately NOT ignored — that one is the team's.
/.claude/settings.local.json

# This machine's harness profiles (commands, config folders). Never committed: a profile's
# command runs on a click, and a merged edit would run it on every machine.
/charter.local.toml
/purlis.local.toml

# Session records of chats at the plane root (a chat's Smart close writes them). This
# machine's own, like a LOCAL workspace's; delete this line to share them with the plane.
/sessions/

# Local skills, at the plane root, in each workspace and of each persona (skills.local/): this
# machine's own, never committed. Move one into skills/ (Share) to give it to the team.
/skills.local/
/workspaces/*/skills.local/
/personas/*/skills.local/
/skills.local.lock

# Python
__pycache__/
*.py[cod]
.venv/

# OS / editor cruft
.DS_Store
";

const LOCAL_SETTINGS_IGNORE: &str = "/.claude/settings.local.json";
/// The `.gitignore` lines that keep the local settings file on this machine, under BOTH its
/// names (RN-2a): a line for a name the plane does not use is inert, and a leftover under the
/// other name would otherwise be committed.
fn local_profiles_ignores() -> Vec<String> {
    old_then_purlis(crate::names::LOCAL_SETTINGS)
        .map(|name| format!("/{name}"))
        .collect()
}

/// A name's window spellings, the old one first: the order the lines have always come in, with
/// the purlis one after it.
fn old_then_purlis(name: crate::names::Name) -> impl Iterator<Item = &'static str> {
    name.reads
        .iter()
        .copied()
        .chain(std::iter::once(name.write))
}

/// The `.gitignore` lines that keep the state folder on this machine, under both its names, for
/// the same reason: a leftover `.purlis/` beside `.charter/` holds a vault registry too.
fn state_ignores() -> Vec<String> {
    old_then_purlis(crate::names::STATE_DIR)
        .map(|name| format!("/{name}/"))
        .collect()
}

/// The plane root's session records (ADR 0064), kept on this machine. A workspace's follow the
/// workspace, LOCAL until it is made LIVE; the plane root has no such switch, so its records
/// take the side that publishes nothing by itself. Un-ignoring the line shares them.
pub const PLANE_SESSIONS_IGNORE: &str = "/sessions/";

/// The local skills of every level (ADR 0099 §1, #2767; a persona's, #3124), kept on this
/// machine: a level's `skills.local/` is read only while git ignores it, and these lines are
/// what make a fresh plane's ignored. They are spelled by the one level rule
/// ([`purlis_skills::local_globs`]), so a level added there is ignored too, and
/// [`GITIGNORE_BASELINE`] holds each (its test says so). A workspace's is already under
/// `/workspaces/*/*`; its line keeps it ignored once the workspace is LIVE and its own `skills/`
/// is un-ignored.
pub fn skills_local_ignores() -> Vec<String> {
    purlis_skills::local_globs()
        .iter()
        .map(|dir| format!("/{dir}/"))
        .chain(["/skills.local.lock".to_owned()])
        .collect()
}

/// A path that resolves somewhere charter will not write: `(the path as named, where it lands)`.
type Escape = (String, String);

/// Everything a run collects before it reports.
#[derive(Default)]
struct Run {
    said: Vec<Say>,
    created: Vec<String>,
    present: Vec<String>,
    /// Baseline paths occupied by something that is not a directory: `(name, path)`.
    blocked: Vec<(String, PathBuf)>,
    /// Paths that resolve out of the plane.
    escapes: Vec<Escape>,
    /// Something asked for failed in a way that has already been said.
    failed: bool,
    /// An ask rule was written, so the workspace layers are rewritten to carry it.
    carry: bool,
    /// Why the forge could not be read from a remote (#839), when it could not.
    asks_forge: Option<String>,
}

impl Run {
    fn info(&mut self, s: impl Into<Markup>) {
        self.said.push(Say::Info(s.into()));
    }
    fn ok(&mut self, s: impl Into<Markup>) {
        self.said.push(Say::Ok(s.into()));
    }
    fn warn(&mut self, s: impl Into<Markup>) {
        self.said.push(Say::Warn(s.into()));
    }
    fn err(&mut self, s: impl Into<Markup>) {
        self.said.push(Say::Err(s.into()));
    }

    /// `rel` under `root`, or `None` with the escape recorded — once per path.
    fn gate(&mut self, root: &Path, rel: &str) -> Option<PathBuf> {
        match gate(root, rel) {
            Ok(path) => Some(path),
            Err(escape) => {
                if !self.escapes.iter().any(|(r, _)| *r == escape.0) {
                    self.escapes.push(escape);
                }
                None
            }
        }
    }

    fn outcome(self, code: u8) -> Outcome {
        Outcome {
            said: self.said,
            code,
            asks_forge: self.asks_forge,
        }
    }
}

/// `root/rel` when it resolves inside the plane and outside its `.git`, else where it lands.
fn gate(root: &Path, rel: &str) -> Result<PathBuf, Escape> {
    let path = root.join(rel);
    let git = root.join(".git");
    let into_git = crate::contain::resolved(&path)
        .is_some_and(|lands| crate::contain::resolved(&git).is_some_and(|g| lands.starts_with(g)));
    if crate::contain::within_plane(root, &path) && !into_git {
        return Ok(path);
    }
    let lands = crate::contain::resolved(&path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "a place purlis cannot resolve (too many links)".to_owned());
    Err((rel.to_owned(), lands))
}

/// Whether anything at all is at `path`, a dangling link included.
fn occupied(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// The words the OS gave for `e`, without Rust's `(os error N)` — Python's `strerror`.
///
/// The OS's own words even for a refusal [`crate::rewrite::refused_write`] reworded: every
/// sentence here names the path already, and the rewording would name it a second time.
fn strerror(e: &std::io::Error) -> String {
    crate::rewrite::os_words(e)
}

/// Whether `root`'s `charter.toml` is a link this command's containment gate refuses: one that
/// leads out of the project or into its `.git`. `init` and `reinit` refuse such a project and
/// write nothing (FR-24); the read-only check leaves it to them, because reading through the
/// link would answer a file that is not the project's.
pub fn manifest_escapes(root: &Path) -> bool {
    escaping_manifest(root).is_some()
}

/// The refusal of a `charter.toml` that is a link out of the project or into its `.git`, in
/// the words that name the link, or `None` when it is not one ([`manifest_escapes`]).
pub fn escaping_manifest(root: &Path) -> Option<String> {
    let (rel, lands) = gate(root, crate::names::manifest_name(root)).err()?;
    if !occupied(&crate::names::manifest(root)) {
        return None;
    }
    Some(format!(
        "{} resolves to {}, which is outside this project or inside its .git — purlis reads \
         and writes nothing through it. Point it inside the project or remove it yourself, then \
         run the command again. Nothing was written.",
        crate::shown::readable(&rel, 1024),
        crate::shown::readable(&lands, 1024)
    ))
}

/// The refusal a command meets on a plane whose format this charter cannot place
/// (`cli._plane_refusal`), or whose `charter.toml` is a link out of it. `init` and `reinit` are
/// not exempt: both write into the plane.
fn refused(root: &Path) -> Option<Outcome> {
    // `charter.toml` is the format gate (V5, FR-24). One that is a link out of the project, or into
    // its `.git`, is somebody else's file: charter reads nothing through it, and writes nothing
    // at all, because every other file would belong to a project whose format it cannot see.
    if let Some(why) = escaping_manifest(root) {
        return Some(Outcome {
            said: vec![Say::Err(why.into())],
            code: 1,
            asks_forge: None,
        });
    }
    match planefile::load(root) {
        planefile::Read::Refused(why) => {
            Some(Outcome {
                said: vec![Say::Err(format!(
                "{why} Nothing was run. `purlis doctor` reports it; `purlis update` is the way \
                 out."
            ).into())],
                code: 1,
                asks_forge: None,
            })
        }
        _ => None,
    }
}

/// `charter init`.
pub fn init(place: &Place, args: &InitArgs) -> Outcome {
    let root = &place.root;
    if let Some(refusal) = refused(root) {
        return refusal;
    }
    let mut run = Run::default();
    if let Some(forge) = args
        .forge
        .as_deref()
        .filter(|forge| !FORGES.contains(forge))
    {
        run.err(format!(
            "unknown --forge {} — known kinds: {}",
            text::py_repr(forge),
            FORGES.join(", ")
        ));
        return run.outcome(1);
    }
    // Refused before anything is written, because it is a question about what the run is FOR:
    // both flags name the source of the first clone, and a charter that silently ranked them
    // would clone one repo while the operator read the other one's name back.
    if args.adopt.is_some() && args.clone_this_repo {
        run.err(
            "--adopt and --clone-this-repo each name the repository to make this plane's first \
             clone, and they name different ones: --clone-this-repo is the repo this plane is \
             being made IN. Ask for one. Nothing was written.",
        );
        return run.outcome(1);
    }

    if let Some(refusal) = repo_is_not_a_plane_yet(root, args) {
        return refusal;
    }

    // Which forge, and whose repos: asked before the first write, and only of a plane whose
    // `charter.toml` this run would write.
    let (forge, owner) = if already_a_plane(root) {
        (String::new(), args.owner.clone())
    } else {
        match forge_and_owner(&mut run, root, args) {
            Some(named) => named,
            None => return run.outcome(ASKS_FOR_FORGE),
        }
    };

    // charter.toml, or purlis.toml once that is the manifest (RN-2a).
    let manifest = crate::names::manifest_name(root);
    if let Some(path) = run.gate(root, manifest) {
        if path.exists() {
            run.present.push(manifest.to_owned());
        } else {
            match std::fs::write(
                &path,
                planefile::render(&forge, &owner, args.host.as_deref()),
            ) {
                Ok(()) => {
                    run.created.push(manifest.to_owned());
                    if owner.is_empty() {
                        run.warn(
                            "No --owner given — charter.toml's [[forge]] block has no \
                             owner/group set. Add one before `purlis discover`.",
                        );
                    }
                }
                Err(e) => {
                    run.err(format!(
                        "could not write {} ({})",
                        path.display(),
                        strerror(&e)
                    ));
                    run.failed = true;
                }
            }
        }
    }

    baseline_dirs(&mut run, root);

    if let Some(path) = run.gate(root, ".gitignore") {
        // One writer of the plane's `.gitignore` at a time: `charter workspace live` edits it
        // too, and each of them rewrites the whole file from what it read.
        let _held = crate::rewrite::Lock::on(root);
        match ensure_gitignore(&path) {
            Ok(true) => run.created.push(".gitignore".to_owned()),
            Ok(false) => run.present.push(".gitignore".to_owned()),
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
    }
    merge_rules(&mut run, root);

    let settings_ok = settings_gate(&mut run, root);
    if settings_ok {
        match settings::ensure_harness_env(root, "claude-code") {
            Wrote::Created => run.created.push(".claude/settings.json (env)".to_owned()),
            // Only an ask or allow rule can meet a deny; an `env` key never does.
            Wrote::Present | Wrote::Denied => {
                run.present.push(".claude/settings.json (env)".to_owned())
            }
            Wrote::Malformed(_) => {}
            Wrote::Blocked(dir) => run.blocked.push((".claude".to_owned(), dir)),
            Wrote::Failed(path, e) => write_failed(&mut run, &path, &e),
        }
    }

    // No ask rule for a handoff (#1444): a handoff is a dispatch, and what consents to one
    // is purlis's own dispatch grant, which the app asks the person for on every harness.
    ask_gate(&mut run, root, settings_ok, &REPORT_ASK, "init");
    ask_gate(&mut run, root, settings_ok, &PROMOTE_ASK, "init");
    purlis_twins(&mut run, root, settings_ok, "init");

    profile_approvals(&mut run, root);

    if let Some(name) = &args.front_door {
        front_door(&mut run, root, name);
    }

    let mut malformed_settings = false;
    if settings_ok {
        match settings::ensure_guard_hook(root, crate::profiles::home().as_deref()) {
            Wrote::Created => run
                .created
                .push(".claude/settings.json (plane-root guard)".to_owned()),
            Wrote::Present | Wrote::Denied => run
                .present
                .push(".claude/settings.json (plane-root guard already wired)".to_owned()),
            Wrote::Malformed(_) => {
                malformed_settings = true;
                let path = root.join(settings::SETTINGS);
                run.warn(Markup::lines([
                    left_untouched(&path).into(),
                    Markup::lines_of(&settings::hooks_snippet()),
                ]));
            }
            Wrote::Blocked(dir) => run.blocked.push((".claude".to_owned(), dir)),
            Wrote::Failed(path, e) => write_failed(&mut run, &path, &e),
        }
    }

    if !run.blocked.is_empty() || !run.escapes.is_empty() || malformed_settings || run.failed {
        report_blockers(&mut run, "init");
        if !run.created.is_empty() {
            let line = format!("  created: {}", run.created.join(", "));
            run.info(line);
        }
        if !run.present.is_empty() {
            let line = format!("  already present: {}", run.present.join(", "));
            run.info(line);
        }
        carry_rules(&mut run, root);
        return run.outcome(1);
    }

    if run.created.is_empty() {
        run.ok(format!(
            "Control plane already fully set up (schema {}) — nothing to do.",
            planefile::WRITTEN
        ));
    } else {
        let entries = fold(&run.created);
        run.ok(format!(
            "Initialized control plane (schema {}) — {} written.",
            planefile::WRITTEN,
            counted(entries.len(), "item", "items")
        ));
        for item in entries {
            run.info(format!("  + {item}"));
        }
    }
    if !run.present.is_empty() {
        let line = format!("  already present: {}", run.present.join(", "));
        run.info(line);
    }
    carry_rules(&mut run, root);
    run.info("Next: `purlis doctor` to preflight, then `purlis discover` to build the inventory.");
    let code = first_clone_step(&mut run, root, args);
    run.outcome(code)
}

/// Why a fix that adds a `.gitignore` line refuses where the file is a symbolic link: the
/// `local-ignore` and `live-skills` fixes say it in these words.
pub(crate) const GITIGNORE_IS_A_LINK: &str = "this project's .gitignore is a symbolic link, \
     which git does not read, so a line added to it would change nothing; nothing was written. \
     Replace the link with a real .gitignore, then run the fix again.";

/// Whether the project's `.gitignore` is a symbolic link. git does not read one, so a line
/// written through it changes nothing git does: the `local-ignore` fix is neither offered nor
/// applied there.
pub fn gitignore_is_a_link(root: &Path) -> bool {
    std::fs::symlink_metadata(root.join(".gitignore")).is_ok_and(|m| m.file_type().is_symlink())
}

/// The doctor's `local-ignore` fix (FX-2): append the local settings file's ignore lines, under
/// both its names (`/charter.local.toml`, `/purlis.local.toml`, RN-2a), to the project's
/// `.gitignore`, creating the file when there is none, and nothing else. The part of
/// [`reinit`] that keeps this machine's harness profiles out of a commit, without the rest of
/// reinit. Every line already in `.gitignore` is kept as it is; a line already there is not
/// added twice. A `.gitignore` that is a link out of the project is refused, writing nothing.
pub fn ignore_local_profiles(root: &Path) -> Outcome {
    if let Some(refusal) = refused(root) {
        return refusal;
    }
    let mut run = Run::default();
    if let Some(path) = run.gate(root, ".gitignore") {
        let _held = crate::rewrite::Lock::on(root);
        let lines = local_profiles_ignores();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        match append_gitignore(
            &path,
            &lines,
            "added by charter's local-ignore fix — harness profiles stay on this machine",
        ) {
            Ok(written) if !written.is_empty() => run
                .created
                .push(format!(".gitignore ({})", written.join(", "))),
            Ok(_) => {}
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
    }
    if !run.escapes.is_empty() || run.failed {
        report_blockers(&mut run, "doctor --fix local-ignore");
        return run.outcome(1);
    }
    if run.created.is_empty() {
        run.ok(format!(
            ".gitignore already names {} — nothing to do.",
            local_profiles_ignores().join(" and ")
        ));
    } else {
        run.ok(format!("added {}.", run.created.join(", ")));
    }
    run.outcome(0)
}

/// `charter reinit`.
pub fn reinit(place: &Place) -> Outcome {
    let root = &place.root;
    if let Some(refusal) = refused(root) {
        return refusal;
    }
    let mut run = Run::default();
    if !place.is_plane {
        run.err(
            "no control plane found (no charter.toml here or in any parent) — `purlis reinit` \
             only works inside one.",
        );
        return run.outcome(1);
    }

    baseline_dirs(&mut run, root);

    if let Some(path) = run.gate(root, ".gitignore") {
        let _held = crate::rewrite::Lock::on(root);
        // The state folder's purlis name too (RN-2a): `/.charter/` came with the baseline,
        // and a leftover `.purlis/` holds a vault registry as much as `.charter/` does.
        let states = state_ignores();
        let missing_state: Vec<&str> = states
            .iter()
            .map(String::as_str)
            .filter(|line| !read_text(&path).is_ok_and(|body| body.contains(&line[1..])))
            .collect();
        if !missing_state.is_empty() {
            match append_gitignore(
                &path,
                &missing_state,
                "added by `charter reinit` — charter's state stays on this machine",
            ) {
                Ok(written) if !written.is_empty() => run
                    .created
                    .push(format!(".gitignore ({})", written.join(", "))),
                Ok(_) => {}
                Err(e) => {
                    run.err(format!("could not read or write {} ({e})", path.display()));
                    run.failed = true;
                }
            }
        }
        let lines = local_profiles_ignores();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        match append_gitignore(
            &path,
            &lines,
            "added by `charter reinit` — harness profiles stay on this machine",
        ) {
            Ok(written) if !written.is_empty() => run
                .created
                .push(format!(".gitignore ({})", written.join(", "))),
            Ok(_) => {}
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
        match append_gitignore(
            &path,
            &[PLANE_SESSIONS_IGNORE],
            "added by `charter reinit` — the plane root's session records stay on this machine",
        ) {
            Ok(written) if !written.is_empty() => run
                .created
                .push(format!(".gitignore ({PLANE_SESSIONS_IGNORE})")),
            Ok(_) => {}
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
        let ignores = skills_local_ignores();
        match append_gitignore(
            &path,
            &ignores.iter().map(String::as_str).collect::<Vec<_>>(),
            "added by `charter reinit` — local skills stay on this machine",
        ) {
            Ok(written) if !written.is_empty() => run
                .created
                .push(format!(".gitignore ({})", written.join(", "))),
            Ok(_) => {}
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
    }
    merge_rules(&mut run, root);

    let settings_ok = settings_gate(&mut run, root);
    if settings_ok {
        match settings::ensure_harness_env(root, "claude-code") {
            Wrote::Created => run.created.push(".claude/settings.json (env)".to_owned()),
            Wrote::Failed(path, e) => write_failed(&mut run, &path, &e),
            _ => {}
        }
    }

    ask_gate(&mut run, root, settings_ok, &REPORT_ASK, "reinit");
    ask_gate(&mut run, root, settings_ok, &PROMOTE_ASK, "reinit");
    purlis_twins(&mut run, root, settings_ok, "reinit");

    profile_approvals(&mut run, root);

    if settings_ok {
        match settings::ensure_guard_hook(root, crate::profiles::home().as_deref()) {
            Wrote::Created => run
                .created
                .push(".claude/settings.json (plane-root guard)".to_owned()),
            Wrote::Present | Wrote::Denied => run
                .present
                .push(".claude/settings.json (plane-root guard already wired)".to_owned()),
            Wrote::Malformed(_) => {
                let path = root.join(settings::SETTINGS);
                run.warn(Markup::lines([
                    left_untouched(&path).into(),
                    Markup::lines_of(&settings::hooks_snippet()),
                ]));
            }
            // Python's `reinit` drops this status on the floor; the `.claude` blocker is
            // reported here rather than lost.
            Wrote::Blocked(dir) => run.blocked.push((".claude".to_owned(), dir)),
            Wrote::Failed(path, e) => write_failed(&mut run, &path, &e),
        }
    }

    if !run.blocked.is_empty() || !run.escapes.is_empty() || run.failed {
        report_blockers(&mut run, "reinit");
        if !run.created.is_empty() {
            let line = format!("  created: {}", run.created.join(", "));
            run.info(line);
        }
        if !run.present.is_empty() {
            let line = format!("  already present: {}", run.present.join(", "));
            run.info(line);
        }
        carry_rules(&mut run, root);
        return run.outcome(1);
    }

    if run.created.is_empty() {
        run.ok(format!(
            "Up to date (schema {}) — nothing to do.",
            planefile::WRITTEN
        ));
        return run.outcome(0);
    }
    let line = format!(
        "Reinitialized control plane → added {}.",
        run.created.join(", ")
    );
    run.ok(line);
    if !run.present.is_empty() {
        let line = format!("  already present: {}", run.present.join(", "));
        run.info(line);
    }
    carry_rules(&mut run, root);
    run.outcome(0)
}

/// `commands._create_baseline_dirs`: create each baseline directory that is absent, and
/// block on one whose path holds something that is not a directory — a file, or a link that
/// leads nowhere. Python crashes on the dangling link (`mkdir` meets `EEXIST`); it is a
/// blocker here like any other thing in the way.
fn baseline_dirs(run: &mut Run, root: &Path) {
    for dir in BASELINE_DIRS {
        let Some(path) = run.gate(root, dir) else {
            continue;
        };
        if path.is_dir() {
            run.present.push(format!("{dir}/"));
        } else if occupied(&path) {
            run.blocked.push((dir.to_owned(), path));
        } else {
            match std::fs::create_dir_all(&path) {
                Ok(()) => run.created.push(format!("{dir}/")),
                Err(e) => write_failed(run, &path, &e),
            }
        }
    }
    workspaces_gitkeep(run, root);
}

/// `workspaces/.gitkeep`: the file the baseline `.gitignore` un-ignores, and the line the
/// LIVE block is spliced in after (#355). Without it an empty `workspaces/` cannot be
/// committed at all, and the splice point depends on whether it happens to be there.
///
/// Created when absent and never truncated, like the persona `.gitkeep`s, and through the
/// same [`touch`]: a link at that name — one inside the plane, or a dangling one, which the
/// gate lets by — is a write failure, as a persona's is (#440). It is part of making
/// `workspaces/`, so it is named on its own only when a plane that already had the directory
/// is given it. A directory at that name is left alone: `workspaces/` still works without it.
fn workspaces_gitkeep(run: &mut Run, root: &Path) {
    const GITKEEP: &str = "workspaces/.gitkeep";
    let dir = root.join("workspaces");
    if !dir.is_dir() {
        return;
    }
    let Some(path) = run.gate(root, GITKEEP) else {
        return;
    };
    match touch(&path) {
        Ok(true) => {
            if !run.created.iter().any(|c| c == "workspaces/") {
                run.created.push(GITKEEP.to_owned());
            }
        }
        Ok(false) => {}
        Err(e) => write_failed(run, &path, &e),
    }
}

/// Whether the settings file may be touched at all: inside the plane, and under a `.claude`
/// that is a directory or absent. A `.claude` FILE is a blocker Python meets only after it
/// has crashed trying to write under it.
fn settings_gate(run: &mut Run, root: &Path) -> bool {
    let Some(dir) = run.gate(root, ".claude") else {
        return false;
    };
    if occupied(&dir) && !dir.is_dir() {
        run.blocked.push((".claude".to_owned(), dir));
        return false;
    }
    run.gate(root, settings::SETTINGS).is_some()
}

/// `Path.touch` for a `.gitkeep`: created when absent, never truncated, and never through a
/// link (#434). `create_new` is `O_EXCL`, which no link survives, so a link planted after the
/// gate answered meets `AlreadyExists` — and is then refused, because a `.gitkeep` that is a
/// link is not one charter made. A plain file already there is left as it is. `true` when it
/// was created.
fn touch(path: &Path) -> std::io::Result<bool> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    match crate::contain::nofollow(&mut options).open(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            match std::fs::symlink_metadata(path) {
                Ok(found) if found.file_type().is_symlink() => Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "{} is a symlink, and purlis will not write through one",
                        path.display()
                    ),
                )),
                _ => Ok(false),
            }
        }
        Err(e) => Err(e),
    }
}

fn write_failed(run: &mut Run, path: &Path, e: &std::io::Error) {
    run.err(format!(
        "could not write {} ({}) — left untouched.",
        path.display(),
        strerror(e)
    ));
    run.failed = true;
}

/// Every blocker, one sentence each, in charter's words where Python has them.
fn report_blockers(run: &mut Run, command: &str) {
    let blocked = std::mem::take(&mut run.blocked);
    for (name, path) in blocked {
        run.err(format!(
            "{name}/ can't be created — {} already exists and is not a directory. purlis never \
             deletes or renames existing content; move or remove it yourself, then re-run \
             `purlis {command}`.",
            path.display()
        ));
    }
    let escapes = std::mem::take(&mut run.escapes);
    for (rel, lands) in escapes {
        run.err(format!(
            "{} resolves to {}, which is outside this plane or inside its .git — purlis reads \
             and writes nothing through it. Point it inside the plane or remove it yourself, \
             then re-run `purlis {command}`.",
            crate::shown::readable(&rel, 1024),
            crate::shown::readable(&lands, 1024)
        ));
    }
}

/// `commands._settings_left_untouched`.
fn left_untouched(path: &Path) -> String {
    format!(
        "{} is not a settings file purlis can read and write back as JSON — left it \
         completely untouched. Wire the plane-root guard yourself:",
        crate::shown::readable(&path.display().to_string(), 1024)
    )
}

/// `commands._fold_entries`: `a.json (x)`, `a.json (y)` → `a.json (x, y)`, order kept.
fn fold(entries: &[String]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut notes: Vec<Vec<String>> = Vec::new();
    for entry in entries {
        let (head, tail) = match entry.split_once(" (") {
            Some((head, tail)) => (head, tail.trim_end_matches(')')),
            None => (entry.as_str(), ""),
        };
        let at = match order.iter().position(|h| h == head) {
            Some(at) => at,
            None => {
                order.push(head.to_owned());
                notes.push(Vec::new());
                order.len() - 1
            }
        };
        if !tail.is_empty() && !notes[at].iter().any(|n| n == tail) {
            notes[at].push(tail.to_owned());
        }
    }
    order
        .into_iter()
        .zip(notes)
        .map(|(head, notes)| {
            if notes.is_empty() {
                head
            } else {
                format!("{head} ({})", notes.join(", "))
            }
        })
        .collect()
}

/// `commands._ensure_gitignore`: add the baseline's rules to `.gitignore` — the whole file
/// when there is none, else only the lines it is missing. True iff it wrote.
///
/// The presence check is Python's to the letter, including its one substring test
/// (`.charter/` anywhere in the body) and its whole-line tests for the rest.
/// The plane's `.gitattributes` merge rules, written through the same gate as `.gitignore`.
fn merge_rules(run: &mut Run, root: &Path) {
    if let Some(path) = run.gate(root, ".gitattributes") {
        let _held = crate::rewrite::Lock::on(root);
        match ensure_gitattributes(&path) {
            Ok(true) => run.created.push(".gitattributes (merge rules)".to_owned()),
            Ok(false) => run.present.push(".gitattributes (merge rules)".to_owned()),
            Err(e) => {
                run.err(format!("could not read or write {} ({e})", path.display()));
                run.failed = true;
            }
        }
    }
}

/// The files a plane only ever grows by whole lines, merged by git's `union` driver so two
/// machines appending to them never conflict (`docs/plane-format.md`, `.gitattributes`).
const MERGE_RULES: [&str; 7] = [
    "personas/_dispatch/*.jsonl merge=union",
    "personas/_skills/*.jsonl merge=union",
    "workspaces/*/pieces/*.jsonl merge=union",
    "workspaces/*/changes/log/*.jsonl merge=union",
    "workspaces/*/work/*.jsonl merge=union",
    "personas/*/memory/MEMORY.md merge=union",
    "workspaces/*/memory/MEMORY.md merge=union",
];

/// The managed block, exactly as the plane format records it, under the markers the plane at
/// `plane` writes ([`names::Name::writes_for`]): charter's until it is migrated (ADR 0051).
fn merge_rules_block(plane: &Path) -> String {
    let mut out = vec![names::MERGE_RULES_BEGIN.writes_for(plane)];
    out.extend(MERGE_RULES);
    out.push(names::MERGE_RULES_END.writes_for(plane));
    out.join("\n")
}

/// Write the managed block into the plane's `.gitattributes`: replace the one there, or add it
/// at the end, keeping every line outside it. `Ok(true)` when the file changed.
///
/// A BEGIN with no END after it is left as it is — half a managed block is something somebody
/// edited — and the block is added below it.
///
/// Either name's markers make the block (V93i): a block is rewritten where it stands, never
/// left beside a second one — under the purlis markers once the plane is migrated, and under
/// charter's until then, so a teammate on an older build still finds its block (V93g).
pub fn ensure_gitattributes(path: &Path) -> Result<bool, String> {
    // A file that is not there is empty; one that cannot be read is said, in the OS's words,
    // rather than written over as if it were empty.
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(strerror(&e)),
    };
    let block = merge_rules_block(path.parent().unwrap_or(Path::new("")));
    // Whole lines only, and only a COMPLETE block: the last BEGIN line with an END line after
    // it. A BEGIN or an END on its own is something somebody edited, and is left exactly as
    // it is — the rules are added below it, and the next run finds that complete block.
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let is =
        |line: &str, marker: &names::Name| marker.recognises(line.trim_end_matches(['\n', '\r']));
    let complete = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is(line, &names::MERGE_RULES_BEGIN))
        .filter_map(|(start, _)| {
            lines[start + 1..]
                .iter()
                .position(|line| is(line, &names::MERGE_RULES_END))
                .map(|offset| (start, start + 1 + offset))
        })
        .next_back();
    let next = match complete {
        Some((start, end)) => {
            let before: String = lines[..start].concat();
            let after: String = lines[end + 1..].concat();
            format!("{before}{block}\n{after}")
        }
        None if text.is_empty() => format!("{block}\n"),
        None => format!("{}\n{block}\n", text.trim_end_matches('\n')),
    };
    if next == text {
        return Ok(false);
    }
    replace_text(path, &next)?;
    Ok(true)
}

fn ensure_gitignore(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        replace_text(path, GITIGNORE_BASELINE)?;
        return Ok(true);
    }
    let body = read_text(path)?;
    // Read as git reads them: an indented line, or one a lone `\r` joins to the last, ignores
    // nothing, so it is not present.
    let lines: Vec<&str> = text::git_lines(&body).map(|line| line.read).collect();
    // The state folder and the local settings under both their names (RN-2a).
    let states = state_ignores();
    let profiles = local_profiles_ignores();
    let mut missing: Vec<&str> = Vec::new();
    if !lines.contains(&"!/workspaces/.gitkeep") {
        missing.extend(["/workspaces/*/*", "!/workspaces/.gitkeep"]);
    }
    for line in &states {
        // `/.charter/` was always found anywhere in the text (`.charter/`), and still is.
        if !body.contains(line.trim_start_matches('/')) {
            missing.push(line);
        }
    }
    if !lines.contains(&LOCAL_SETTINGS_IGNORE) {
        missing.push(LOCAL_SETTINGS_IGNORE);
    }
    for line in &profiles {
        if !lines.contains(&line.as_str()) {
            missing.push(line);
        }
    }
    if !lines.contains(&PLANE_SESSIONS_IGNORE) {
        missing.push(PLANE_SESSIONS_IGNORE);
    }
    let ignores = skills_local_ignores();
    for line in &ignores {
        if !lines.contains(&line.as_str()) {
            missing.push(line);
        }
    }
    if missing.is_empty() {
        return Ok(false);
    }
    append_gitignore(path, &missing, "added by `charter init`")?;
    Ok(true)
}

/// Replace a plane file `init` or `reinit` edits, whole or not at all (#358): a temp file
/// beside it, then one rename, so a crash never leaves the plane's `.gitignore` cut short.
///
/// A link the caller's gate let through — one that stays inside the plane — is followed, as
/// it always was: the file it lands on is the one replaced, and the link is left a link.
///
/// A failure names the file it lands on as well, when that is not `path` (#1421): the caller's
/// sentence names `path`, the link, and the file refused is the one it leads to.
fn replace_text(path: &Path, text: &str) -> Result<(), String> {
    let target = linked_to(path);
    let dir = target.parent().unwrap_or(Path::new("."));
    crate::rewrite::replace(dir, &target, text.as_bytes(), crate::rewrite::Mode::Kept).map_err(
        |e| {
            let words = strerror(&e);
            if target == path {
                words
            } else {
                format!(
                    "{words}, writing {}, where the link leads",
                    target.display()
                )
            }
        },
    )
}

/// Where `path` lands when it is a symlink, else `path` itself.
pub(super) fn linked_to(path: &Path) -> PathBuf {
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.file_type().is_symlink() => {
            crate::contain::resolved(path).unwrap_or_else(|| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| strerror(&e))?;
    String::from_utf8(bytes).map_err(|_| "it is not UTF-8 text".to_owned())
}

/// `util.append_gitignore`: append the `lines` the file is missing under a `# header`, and
/// return the ones written. Append-only and whole-line: `workspace.set_live` splices its
/// block at the literal `!/workspaces/.gitkeep`, so a writer that rewrote or reordered would
/// break live workspaces from across the codebase.
///
/// A line counts as present only as git reads it ([`text::git_lines`]): an indented copy, or
/// one a lone `\r` joins to the line before, is not, and is added. Trailing line breaks are
/// collapsed to the one blank line before the new block, as Python does; every other byte is
/// kept, and what is added is ended as the file's first line is, so a CRLF file stays CRLF.
pub(crate) fn append_gitignore(
    path: &Path,
    lines: &[&str],
    header: &str,
) -> Result<Vec<String>, String> {
    let body = if path.exists() {
        read_text(path)?
    } else {
        String::new()
    };
    let (next, missing) = with_appended(&body, lines, header);
    if missing.is_empty() {
        return Ok(missing);
    }
    replace_text(path, &next)?;
    Ok(missing)
}

/// [`append_gitignore`]'s text, without the write: `body` with the `lines` it is missing
/// appended under a `# header`, and the lines that were. `body` itself when none is.
///
/// Ended as `body` is: a file whose first line ends `\r\n` gets `\r\n` for the blank line,
/// the header and each line, so a CRLF `.gitignore` is never left with bare `\n` in it.
pub(crate) fn with_appended(body: &str, lines: &[&str], header: &str) -> (String, Vec<String>) {
    let present: Vec<&str> = text::git_lines(body).map(|line| line.read).collect();
    let missing: Vec<String> = lines
        .iter()
        .filter(|l| !present.contains(*l))
        .map(|l| (*l).to_owned())
        .collect();
    if missing.is_empty() {
        return (body.to_owned(), missing);
    }
    let end = match body.find('\n') {
        Some(at) if body[..at].ends_with('\r') => "\r\n",
        _ => "\n",
    };
    let prefix = if crate::memstore::py_strip(body).is_empty() {
        String::new()
    } else {
        format!("{}{end}{end}", body.trim_end_matches(['\n', '\r']))
    };
    let block: String = missing.iter().map(|l| format!("{l}{end}")).collect();
    (format!("{prefix}# {header}{end}{block}"), missing)
}

/// An ask rule `init` writes by default, in each harness's own syntax (#363 D11).
struct AskRule {
    /// What the run's lines call it: `ask: <label>`.
    label: &'static str,
    /// Claude Code's rule.
    rule: &'static str,
    /// opencode's `permission.bash` glob.
    pattern: &'static str,
}

/// The consent rule for filing a report (ADR 0059, amended 2026-09-26): the harness asks the
/// operator before `charter report … --yes <digest>` files anything.
const REPORT_ASK: AskRule = AskRule {
    label: "charter report --yes",
    rule: settings::REPORT_RULE,
    pattern: settings::REPORT_PATTERN,
};

/// The consent rule for promoting a todo to an issue (V42, ADR 0088 §5): the harness asks the
/// operator before `charter ws todo promote` sends a todo's title and text to a forge.
const PROMOTE_ASK: AskRule = AskRule {
    label: "charter ws todo promote",
    rule: settings::PROMOTE_RULE,
    pattern: settings::PROMOTE_PATTERN,
};

/// The consent rule for filing a report, spelt `purlis`.
const PURLIS_REPORT_ASK: AskRule = AskRule {
    label: "purlis report --yes",
    rule: settings::PURLIS_REPORT_RULE,
    pattern: settings::PURLIS_REPORT_PATTERN,
};

/// The consent rule for promoting a todo, spelt `purlis`.
const PURLIS_PROMOTE_ASK: AskRule = AskRule {
    label: "purlis ws todo promote",
    rule: settings::PURLIS_PROMOTE_RULE,
    pattern: settings::PURLIS_PROMOTE_PATTERN,
};

/// Each charter consent rule with its `purlis` twin. The handoff rule is not one (#1444):
/// a project that still carries `charter handoff *` is not given `purlis handoff *` beside it.
const CONSENT_TWINS: [(&AskRule, &AskRule); 2] = [
    (&REPORT_ASK, &PURLIS_REPORT_ASK),
    (&PROMOTE_ASK, &PURLIS_PROMOTE_ASK),
];

/// The `purlis` twin of every consent rule the project asks under its `charter` spelling, in
/// every harness (RN-7, closing D-RN3-4 and D-RN3-9): with both, `purlis report --yes` waits
/// for the operator as `charter report --yes` does, and the guard lets the new spelling through
/// ([`crate::consentspelling`]). A `charter` rule the operator made a deny, or one the project
/// lacks, gets no twin: an ask beside a deny would let the command through after one click.
fn purlis_twins(run: &mut Run, root: &Path, settings_ok: bool, command: &str) {
    for (charter, twin) in CONSENT_TWINS {
        if settings::asks_in_every_harness(root, charter.pattern) {
            ask_gate(run, root, settings_ok, twin, command);
        }
    }
}

/// `commands.ensure_handoff_gate`, for each of charter's default ask rules: the rule in every
/// harness that can hold one, or in none of them. Every file is asked first with nothing
/// written, and one that cannot take the rule stops the whole write — a rule in force under
/// one harness and not another is the split `charter guard` exists to prevent.
///
/// `command` is the command the run is (`init` or `reinit`). `reinit` lists what it added
/// only, so a rule already there is not said. A rule written now is carried into every
/// workspace layer charter generates, as `charter guard ask` carries it (#449).
fn ask_gate(run: &mut Run, root: &Path, settings_ok: bool, ask: &AskRule, command: &str) {
    let opencode_ok = run.gate(root, settings::OPENCODE).is_some();
    if !settings_ok || !opencode_ok {
        // The blocker is reported where it was found; nothing is written anywhere.
        return;
    }
    let checked = [
        (
            "claude-code",
            settings::ensure_ask_rule(root, ask.rule, true),
        ),
        (
            "opencode",
            settings::ensure_opencode_ask(root, ask.pattern, true),
        ),
    ];
    let bad: Vec<&str> = checked
        .iter()
        .filter_map(|(_, w)| match w {
            Wrote::Malformed(detail) => Some(detail.as_str()),
            _ => None,
        })
        .collect();
    if !bad.is_empty() {
        let line = format!(
            "the ask rule for `{}` was not written anywhere — {} is not valid, and \
             purlis writes every harness or none. Fix it, then run purlis {command} again",
            ask.label,
            bad.join(", ")
        );
        run.warn(line);
        return;
    }
    let mut added = false;
    for (name, check) in checked {
        let (rel, wrote) = match (name, check) {
            ("claude-code", Wrote::Created) => (
                settings::SETTINGS,
                settings::ensure_ask_rule(root, ask.rule, false),
            ),
            ("opencode", Wrote::Created) => (
                settings::OPENCODE,
                settings::ensure_opencode_ask(root, ask.pattern, false),
            ),
            ("claude-code", other) => (settings::SETTINGS, other),
            (_, other) => (settings::OPENCODE, other),
        };
        match wrote {
            Wrote::Created => {
                added = true;
                run.created.push(format!("{rel} (ask: {})", ask.label));
            }
            Wrote::Present | Wrote::Denied if command == "reinit" => {}
            Wrote::Present => run.present.push(format!("{rel} (ask: {})", ask.label)),
            Wrote::Denied => run
                .present
                .push(format!("{rel} (denies {}, left so)", ask.label)),
            Wrote::Failed(path, e) => {
                run.err(format!(
                    "{name}: could not write {} ({}) — left untouched.",
                    path.display(),
                    strerror(&e)
                ));
                run.info(
                    "  purlis asked every harness before writing any of them, so this arrived \
                     AFTER the check. Re-run once that file can be written.",
                );
            }
            Wrote::Blocked(dir) => run.blocked.push((".claude".to_owned(), dir)),
            Wrote::Malformed(_) => {}
        }
    }
    run.carry |= added;
}

/// When the run wrote an ask rule, rewrite every workspace layer charter generates so the rule
/// is in force there now, as `charter guard ask` does (#449), and say where it went. Said last,
/// after what the run wrote in the plane.
fn carry_rules(run: &mut Run, root: &Path) {
    if !run.carry {
        return;
    }
    for line in crate::guardcmd::mirror(root).lines() {
        match line.strip_prefix("! ") {
            Some(warning) => run.warn(warning.to_owned()),
            None => run.info(line.to_owned()),
        }
    }
}

/// `commands._wire_profiles`, as much of it as is left: one line for each DECLARED profile
/// nobody has approved yet.
///
/// Python's version also wired each profile's own config folder — `claude plugin install`,
/// Codex's marketplace, opencode's shim. The app wires nothing there: it arms every chat it
/// starts with its own plugin, for that session alone (`crate::plugin`), so there is no folder
/// to wire and nothing for `init` or `reinit` to report about one. What still stops a chat is
/// a command the operator has not approved, and that is worth a line here.
fn profile_approvals(run: &mut Run, root: &Path) {
    use crate::profiles::{self, Source};
    if !profiles::ignore_check(root).passes() {
        return;
    }
    for p in profiles::current(root).profiles() {
        if p.source == Source::BuiltIn {
            continue;
        }
        if let Some(state) = crate::profiletrust::approval_needed(root, p) {
            let name = crate::shown::short(&p.name);
            run.warn(format!(
                "  profile '{name}' is {} and not approved yet — start a chat on it from the \
                 app's new-chat picker to approve its command",
                state.as_str()
            ));
        }
    }
}

/// `^[a-z0-9][a-z0-9._-]*$` — a name `persona.valid_name` accepts. `_shared` is not one.
fn persona_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// The persona file the front door is scaffolded with (`commands._FRONT_DOOR`).
fn front_door_text(name: &str, role: &str, program: &str) -> String {
    format!(
        r#"---
name: {name}
role: {role}
vault: none
delegate-when: routing work to the right persona, and scoping a request before code is written
---

# {role}

You are the front door of this control plane. Your job is to understand what is actually
being asked, then either do it or hand it to the persona that owns it — not to start
editing the first file that looks relevant.

## Routing

Route to this plane's other personas by what each one's `delegate-when` claims.
`{program} persona list` names them; each is `personas/<name>/persona.md`. {program} never says
which one owns the request — that call is yours. Route on the *work*, not on the file a
change happens to touch.

If you are the only one, that is the first thing worth fixing, not a reason to do
everything here:

```
personas/<name>/persona.md
---
role: <Role>
delegate-when: <the work that should come to it>
---
```

`delegate-when` is what makes a persona findable — it becomes the description whoever is
routing reads. Create one the moment a second kind of work appears in this plane.

Cross-cutting changes stay with you: splitting one coherent change across three personas
costs more in lost context than it saves.

## Scout before you scope

Read the thing before proposing a change to it, and check what this plane already knows —
`{program} recall "<keywords>"` searches your memory, the shared namespace and the active
workspace's journal at once.

## What you own

Personas, workspaces, memory and vaults — the shape of this plane. Definitions and memory
are committed and shared; credentials never are.

Record durable facts with the `persona_remember` tool, or
`{program} persona remember {name} "<fact>"`; `shared` (`--shared`) for anything every persona
needs.

This file is yours: rename it, rewrite it, or delete it and declare a different front door
with `{program} persona default <name>`.
"#
    )
}

/// `commands._ensure_front_door`: scaffold one generic persona and declare it — only on a
/// plane with no persona at all and no declared default. A roster is not absent because one
/// particular name is, and a declared default is a question somebody already answered.
fn front_door(run: &mut Run, root: &Path, name: &str) {
    let Some(personas) = run.gate(root, "personas") else {
        return;
    };
    if !personas.is_dir() {
        // Blocked, and said so where the baseline was made. Python goes on to `mkdir` under
        // the file and ends in a traceback.
        return;
    }
    if let Ok(entries) = std::fs::read_dir(&personas) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_md = path.extension().is_some_and(|e| e == "md");
            if occupied(&path.join("persona.md")) || is_md {
                return;
            }
        }
    }
    if run.gate(root, crate::names::manifest_name(root)).is_none() {
        return;
    }
    match planefile::load(root) {
        planefile::Read::Config(cfg) if planefile::declares_default_persona(&cfg) => return,
        planefile::Read::Malformed(why) => {
            // Python raises here and ends `init` with a traceback, leaving the guard hook
            // unwritten. Said, and the rest of `init` goes on.
            run.err(format!("{why} — no front door was scaffolded."));
            run.failed = true;
            return;
        }
        _ => {}
    }
    if !persona_name_ok(name) {
        run.warn(format!(
            "--front-door {} is not a valid persona name — skipped.",
            text::py_repr(name)
        ));
        return;
    }
    // The only place this binary MINTS a persona directory, and `persona.valid_name`'s
    // alphabet above admits `nul`, `con` and `com1` — measured, charter-app#96. A persona is
    // committed, so the name travels; the refusal is here and not in
    // `contain::persona_name_ok`, which every reader of an existing plane asks.
    if let Err(why) = crate::contain::mintable(name) {
        run.warn(format!(
            "--front-door {} would not name the same directory on every machine this plane \
             reaches — {why} — skipped.",
            text::py_repr(name)
        ));
        return;
    }
    let rel = format!("personas/{name}");
    let files = [
        format!("{rel}/persona.md"),
        format!("{rel}/memory/.gitkeep"),
        format!("{rel}/refs/.gitkeep"),
    ];
    if run.gate(root, &rel).is_none() || files.iter().any(|f| run.gate(root, f).is_none()) {
        return;
    }
    let role = text::py_title(&name.replace(['-', '_'], " "));
    fn scaffold(dir: &Path, name: &str, role: &str, program: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        // Never through a link (#434): the paths were gated above — a link on the way that
        // stays inside the plane is followed, as it always was — and a link that lands at the
        // file in between is refused or replaced rather than written through.
        crate::rewrite::replace(
            dir,
            &dir.join("persona.md"),
            front_door_text(name, role, program).as_bytes(),
            crate::rewrite::Mode::Kept,
        )?;
        for sub in ["memory", "refs"] {
            std::fs::create_dir_all(dir.join(sub))?;
            touch(&dir.join(sub).join(".gitkeep"))?;
        }
        Ok(())
    }
    // The persona is committed: it names the program as the plane spells it (D-RN11a-1).
    let program = crate::names::BINARY.writes_for(root);
    if let Err(e) = scaffold(&root.join(&rel), name, &role, program) {
        write_failed(run, &root.join(&rel), &e);
        return;
    }
    // Python ignores a declaration that could not be written (`_try_set_key`'s `False`), and
    // so does this: the persona is on disk either way, and `doctor` names a front door that
    // is not declared.
    let _ = planefile::set_key(root, "persona", "default", name);
    run.created.push(format!(
        "personas/{name}/ (front door, declared in charter.toml)"
    ));
}

/// The flags the operator typed, as a shell will read them back — so the two commands the
/// refusal below prints can be pasted rather than reassembled by hand.
///
/// Only what `init` was actually given: `--forge`, `--owner` and `--host` when they carry
/// something (with no `--forge`, the commands below read it from the repo's origin); and the
/// front-door flags only when they are not the default, which is the one `None` that means
/// `--no-front-door` rather than "unset".
fn typed_flags(args: &InitArgs) -> String {
    use crate::handoff::quote;
    let mut out = String::new();
    if let Some(forge) = &args.forge {
        out.push_str(&format!(" --forge {}", quote(forge)));
    }
    if !args.owner.is_empty() {
        out.push_str(&format!(" --owner {}", quote(&args.owner)));
    }
    if let Some(host) = &args.host {
        out.push_str(&format!(" --host {}", quote(host)));
    }
    match &args.front_door {
        None => out.push_str(" --no-front-door"),
        Some(name) if name != "steward" => {
            out.push_str(&format!(" --front-door {}", quote(name)));
        }
        Some(_) => {}
    }
    out
}

/// **The declared divergence from the Python oracle** (ADR 0035, spec decision 27): `charter
/// init` does not make an existing git repository into a control plane unless it is asked to.
///
/// The old default — Python's, still — wrote `charter.toml`, `personas/`, `inventory/`,
/// `workspaces/` and a block of rules into that repository's own tracked `.gitignore`, and
/// only then printed the offer to clone it into the plane's first workspace. ADR 0033 turned
/// "which plane" into a directory picked in a file dialog, and `_is_repo_top_level`'s own
/// docstring is what that breaks: the offer *"exists for one person standing in one project,
/// which is the equality case"*. A directory picked from a list has nobody standing in it, so
/// the scaffolding and the `.gitignore` edit are writes the operator never typed.
///
/// So the safe shape is the default and the old one is asked for by name. **Nothing is
/// written**: this runs before the first `mkdir`, because the write that was never typed is
/// the whole subject.
///
/// Three cases are exempt, one reason each:
///
/// - `--plane-is-this-repo` IS the operator saying so;
/// - `--clone-this-repo` says it too — it asks for a clone INTO the plane this makes here,
///   which is not a request anyone types by accident;
/// - a directory that already holds a `charter.toml` is a plane, where the question was
///   answered once and `init` is now a heal. charter's own plane is that case, and so is
///   every re-run — `init` is idempotent and gets run again after a forge is added or a
///   charter is upgraded, and a refusal there would break the one flow this repo is built in.
///
/// A repository this merely sits INSIDE is untouched by any of it, exactly as the offer was:
/// a `$HOME` kept under git would otherwise refuse to hold a plane at all.
fn repo_is_not_a_plane_yet(root: &Path, args: &InitArgs) -> Option<Outcome> {
    if args.plane_is_this_repo || args.clone_this_repo {
        return None;
    }
    if already_a_plane(root) {
        return None;
    }
    if !is_repo_top_level(root) {
        return None;
    }
    let name = first_clone_name(root);
    let flags = typed_flags(args);
    let mut run = Run::default();
    run.err(format!(
        "this is the git repo '{name}', and `purlis init` does not make a repository into a \
         control plane unless you ask it to. Nothing was written."
    ));
    // Two commands, not three, and the second one ADOPTS rather than describing an adoption:
    // `--adopt` clones this repo into the new plane's first workspace (charter-app#175). The
    // directory is named rather than guessed — `init` writes nothing outside the plane it was
    // pointed at, and picking `../<name>-plane` for the operator would be that write.
    let here = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    run.info(Markup::lines([
        "A plane is a directory of its own, and this repo is the first clone in it:".into(),
        format!("      mkdir ../{name}-plane && cd ../{name}-plane").into(),
        format!("      purlis init{flags} --adopt ../{here}").into(),
        "  That clones this repo into the plane's first workspace. Nothing here is written by \
         any of it — this repo is read, and only read."
            .into(),
    ]));
    // Named in full rather than summarised: this is the line somebody decides on, and "plane
    // scaffolding" is not a list anyone can picture. Built from the same constants the writes
    // come from, so a path that moves cannot leave this sentence behind.
    let written = [
        crate::names::PLANE_MANIFEST.newest_old(),
        settings::SETTINGS,
        settings::OPENCODE,
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(BASELINE_DIRS.iter().map(|d| format!("{d}/")))
    .collect::<Vec<_>>();
    run.info(Markup::lines([
        "To make THIS repo the plane instead — purlis's own plane is one, which is why the \
         option is here — ask for it by name:"
            .into(),
        format!("      purlis init --plane-is-this-repo{flags}").into(),
        format!(
            "  That writes {} into this repo, and purlis's own rules into its tracked \
             .gitignore.",
            written.join(", ")
        )
        .into(),
    ]));
    run.info(
        "Why the default changed: docs/adr/0035-a-plane-is-untrusted-until-the-operator-opens-\
         it.md, and charter-app spec decision 27. `purlis init` anywhere that is not the top \
         of a git repo is unchanged.",
    );
    Some(run.outcome(1))
}

/// The forge and owner `charter.toml` is written with (#839): what the operator named, else
/// what the `origin` of the repo the plane is made for says. `None` when neither says, after
/// asking for it on `run`.
///
/// The repo is the one `--adopt` names, or the plane's own directory for `--clone-this-repo`
/// and `--plane-is-this-repo`. A plain `init` names no repo, so it is asked.
fn forge_and_owner(run: &mut Run, root: &Path, args: &InitArgs) -> Option<(String, String)> {
    let repo = match &args.adopt {
        Some(repo) => Some(repo.clone()),
        None if args.clone_this_repo || args.plane_is_this_repo => Some(root.to_path_buf()),
        None => None,
    };
    let read = match &repo {
        Some(repo) => fromremote::forge_of_repo(repo),
        None => Err("no repo was named to read it from".to_owned()),
    };
    if let Some(forge) = &args.forge {
        // `--owner`'s help: the owner comes from the same origin when it is not given, and
        // only from an origin on the forge that was named.
        let owner = match &read {
            Ok(found) if args.owner.is_empty() && found.kind.word() == forge => found.owner.clone(),
            _ => args.owner.clone(),
        };
        return Some((forge.clone(), owner));
    }
    match read {
        Ok(found) => {
            let owner = if args.owner.is_empty() {
                found.owner
            } else {
                args.owner.clone()
            };
            let whose = if owner.is_empty() {
                String::new()
            } else {
                format!(", owner {owner}")
            };
            run.info(format!(
                "Forge {}{whose}, read from {}'s origin.",
                found.kind.word(),
                repo.as_deref().unwrap_or(root).display()
            ));
            Some((found.kind.word().to_owned(), owner))
        }
        Err(why) => {
            run.err(format!(
                "purlis cannot tell which forge this project's repos are on: {why}. Name it \
                 with `--forge github` or `--forge gitlab`, and its org or group with \
                 `--owner`, or adopt a repo whose origin is on github.com or gitlab.com with \
                 `--adopt <repo>`. Nothing was written."
            ));
            run.asks_forge = Some(why);
            None
        }
    }
}

/// Whether `root` already holds a `charter.toml` charter would read — asked through the gate,
/// so a manifest that is a link out of the plane does not count as one. It is not a plane
/// charter can heal, and answering "no" here only costs that run its scaffolding, which is
/// the side to be wrong on.
fn already_a_plane(root: &Path) -> bool {
    gate(root, crate::names::manifest_name(root)).is_ok_and(|path| path.exists())
}

/// `commands._first_clone_step`, plus the source `--adopt` names: the one thing a repository
/// changes about `init` — what it OFFERS, and now what it can be asked to do. Nothing is ever
/// cloned that was not asked for by name.
fn first_clone_step(run: &mut Run, root: &Path, args: &InitArgs) -> u8 {
    let now = args.now.unwrap_or_else(chrono::Utc::now);
    if let Some(repo) = &args.adopt {
        return match adopted(root, repo) {
            Ok(source) => firstclone::into_first_workspace(run, root, &source, now),
            Err(why) => {
                run.err(why);
                1
            }
        };
    }
    let accepted = args.clone_this_repo;
    let here = is_repo_top_level(root);
    if !accepted {
        if here {
            let name = first_clone_name(root);
            let ws = match planefile::load(root) {
                planefile::Read::Config(cfg) => planefile::default_workspace(&cfg),
                _ => "default".to_owned(),
            };
            if !root.join("workspaces").join(&ws).join(&name).exists() {
                run.info(Markup::lines([
                    format!(
                        "You are standing in the git repo '{name}'. Work happens in a \
                         workspace, not in the plane root — clone it into the first one:"
                    )
                    .into(),
                    "      purlis init --clone-this-repo".into(),
                    format!(
                        "  Nothing is cloned unless you run that. It lands in \
                         workspaces/{ws}/{name}/, and declining leaves this plane complete."
                    )
                    .into(),
                ]));
            }
        }
        return 0;
    }
    if !here {
        run.err(format!(
            "--clone-this-repo: there is no repo here to clone. {} is not the top level of a \
             git working tree, and the flag clones the repo you are standing in. The control \
             plane itself was still created.",
            root.display()
        ));
        return 1;
    }
    firstclone::into_first_workspace(run, root, root, now)
}

/// The repository `--adopt` may take as the plane's first clone, resolved, or why it may not
/// be one.
///
/// Three refusals, one reason each, and all three are about a path the operator typed or
/// picked in a dialog rather than one charter derived:
///
/// - **Not the top of a git working tree.** `--adopt` clones a repository; a directory that
///   is merely inside one would clone the whole enclosing repo under the name of a
///   subdirectory, which is `is_repo_top_level`'s own subject one door along.
/// - **The plane root itself.** That is `--clone-this-repo`, which already exists and says so;
///   two spellings of one act is how they drift apart.
/// - **The plane is INSIDE it.** The clone would then land inside the repository being cloned
///   — a write into somebody's repo, which is the exact thing ADR 0035 reversed the default
///   to prevent, arriving through the flag that was supposed to be the safe way.
fn adopted(root: &Path, repo: &Path) -> Result<PathBuf, String> {
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let source = canon(repo);
    if !is_repo_top_level(&source) {
        return Err(format!(
            "--adopt: {} is not the top level of a git working tree, and adopting a repository \
             is what the flag does. The control plane itself was still created.",
            source.display()
        ));
    }
    if source == canon(root) {
        return Err(
            "--adopt: that is this plane's own directory. To clone the repo the plane is \
             being made IN, ask for `--clone-this-repo`; `--adopt` is for a repository \
             somewhere else, with the plane beside it (ADR 0035)."
                .to_owned(),
        );
    }
    if canon(root).starts_with(&source) {
        return Err(format!(
            "--adopt: this plane is inside {}, so cloning that repo would write the clone into \
             the repository it came from. Make the plane in a directory of its own, beside the \
             repo. The control plane itself was still created.",
            source.display()
        ));
    }
    Ok(source)
}

/// `commands._is_repo_top_level`: `root` is the TOP of a git working tree — not merely
/// inside one, because a `$HOME` kept under git would otherwise have `init` offering to
/// clone the home directory.
fn is_repo_top_level(root: &Path) -> bool {
    use crate::worktree::git;
    let Ok(top) = git::run(root, &["rev-parse", "--show-toplevel"], git::READ) else {
        return false;
    };
    // `||` rather than `&&` is not observable, and `.cargo/mutants.toml` excludes that mutant:
    // either way a run that falls through compares `top.line()` with the resolved root, and
    // an empty line (`canonicalize("")` fails, leaving `""`) or a failed git's empty stdout
    // never equals a real directory. The guard is Python's `returncode != 0 or not top`.
    if !top.ok() || top.line().is_empty() {
        return false;
    }
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(Path::new(top.line())) == canon(root)
}

/// `commands._first_clone_name`: the basename of `origin`'s URL, or the directory's name
/// when that is not a workspace-shaped name.
fn first_clone_name(root: &Path) -> String {
    use crate::worktree::git;
    let url = git::run(root, &["remote", "get-url", "origin"], git::READ)
        .ok()
        .map(|r| crate::memstore::py_strip(&r.out).to_owned())
        .unwrap_or_default();
    let tail = url
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("");
    let tail = tail.strip_suffix(".git").unwrap_or(tail);
    if !url.is_empty() && crate::contain::workspace_name_ok(tail) {
        tail.to_owned()
    } else {
        root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;

/// `repo` cloned into a workspace of the plane at `root` named after it (FR-4): the workspace's
/// name and the clone's path, or why nothing was cloned. The words are the window's (ADR 0072:
/// a code repo is a "repo", and the plane is a "project").
///
/// The steps `init --adopt` takes (`firstclone`), with the workspace named after the repo
/// instead of the plane's default one. The repo is read, and only read.
///
/// - **A repo already taken in answers with what is there**: a workspace `<name>` (or
///   `<name>-2`, …) whose clone came from the same place — the same origin, or the same
///   directory when there is none.
/// - **A different repo with the same name gets the next free suffix**, `svc-2`, never the
///   first one's workspace.
/// - **A repo that already IS a clone in this plane's workspaces** is that workspace, and
///   nothing is cloned.
pub fn adopt_as_workspace(
    root: &Path,
    repo: &Path,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(String, PathBuf), String> {
    let unread = |p: &Path, why: std::io::Error| {
        format!(
            "charter cannot read {} ({why}), so nothing was copied.",
            p.display()
        )
    };
    let source = repo.canonicalize().map_err(|why| unread(repo, why))?;
    let root = root.canonicalize().map_err(|why| unread(root, why))?;
    if !is_repo_top_level(&source) {
        return Err(format!(
            "{} is not the top level of a git repo, so there is nothing to open. Pick the \
             folder a repo was cloned into.",
            source.display()
        ));
    }
    if let Ok(inside) = source.strip_prefix(root.join("workspaces"))
        && let Some(ws) = inside.components().next()
    {
        return Ok((ws.as_os_str().to_string_lossy().into_owned(), source));
    }
    if root.starts_with(&source) {
        return Err(format!(
            "{} holds this project, so copying it would write into the repo it came from. Pick \
             another repo.",
            source.display()
        ));
    }
    let Some(base) = crate::contain::workspace_name(&first_clone_name(&source)) else {
        return Err(format!(
            "charter cannot make a workspace name out of {}. Rename the folder, or use New \
             project → Advanced.",
            source.display()
        ));
    };
    let from = firstclone::upstream_of(&source);
    let name = (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|name| {
            let clone = root.join("workspaces").join(name).join(name);
            !clone.exists() || firstclone::origin_of(&clone).as_deref() == Some(from.as_str())
        })
        .expect("an unbounded range always has a free name");
    let mut run = Run::default();
    firstclone::into_workspace(&mut run, &root, &source, &name, &name, now)
        .map(|clone| (name, clone))
}
