// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter save`: git for the control plane itself — staging, the secret guard, committing,
//! and pushing over the plane's OWN forge's HTTPS token. A port of `charter/planegit.py` and
//! `charter/commands.py`'s `cmd_save`.
//!
//! # `save` commits whatever is pending, and says so before it does
//!
//! `charter save` runs `git add -A` in the plane. That is charter's documented behaviour and
//! it is a foot-gun: measured on the operator's own plane on 2026-09-12, a save reported "169
//! file(s)" — which read as a large but ordinary batch of persona memories — and carried a
//! `uv.lock` that had never existed in that repository, created by some agent's local `uv run`
//! and never gitignored. A project file reached `main` with no pull request and nobody
//! choosing it, and the only visible evidence was a count.
//!
//! **A count is not a description**, so this port prints the directory breakdown of what is
//! staged BEFORE the commit is made. It is the one thing the incident's own write-up says is
//! the whole check, and it is the only block this command has that Python does not. Being
//! charter's alone is not a reason for nothing to check it: the differential harness takes it
//! out of the Rust stderr (`rust_only_lines`) and then compares the block it took out, byte
//! for byte, against what the scenario declares (`rust_only_block`) — the headline's counts,
//! every row's count and directory, the order they are in, and the tail that says how many
//! directories were not listed. The behaviour itself is ported faithfully:
//! `save` still stages everything, because narrowing it would silently stop saving things
//! planes rely on it saving.
//!
//! # Where the credential is, and where it is not
//!
//! The token itself never enters this process. Every push goes through
//! [`crate::worktree::git::run_network`], which hands git ONE credential helper — the forge
//! CLI's, by absolute path — and resets every other one, refuses SSH, `ext::`, `git://` and
//! plain `http://`, and passes the forge CLI's own credential environment to the child
//! without ever reading it here. So:
//!
//! - **argv** carries a helper COMMAND, never a secret;
//! - **an error** is git's stderr, and the URL charter pushes to is one it BUILT from the
//!   forge's own HTTPS base rather than one it was handed. A remote carrying userinfo
//!   (`https://x:tok@host/…`) is not that URL, so [`origin_https`] answers `None` and the
//!   warning that follows names no URL at all: a token an operator put in their remote is
//!   neither pushed with nor printed;
//! - **a commit** is guarded by [`crate::secretshape`]: a staged memory or ref file that looks
//!   like it holds a credential stops the save.
//!
//! # Where this is stricter than Python, on purpose
//!
//! - **The plane's own git hooks do not run for charter's commit.** Every call goes through
//!   the hardened runner, which sets `core.hooksPath=/dev/null`. That is the same policy that
//!   turns signing off: a `pre-commit` that prompts, builds or waits hangs an autonomous agent
//!   exactly the way a signer prompt does, and a hook charter triggers is not one the operator
//!   asked for. Python runs them.
//! - **A memory or ref file that leads out of the plane is refused rather than read through.**
//!   Python reads it and scans whatever is on the other end; charter #442 is what a read
//!   through a committed link costs, and a memory file charter cannot read is not one it will
//!   commit unexamined.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::counts::{counted, plural};
use crate::forge::{self, Forge};
use crate::gitstate;
use crate::repocmd::{Say, Sink};
use crate::secretshape;
use crate::shown;
use crate::worktree::git;

/// What a push of the plane root's HEAD did, in one word. Python's module constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It landed on the branch HEAD is on.
    Pushed,
    /// The branch requires a pull request → it landed on `purlis/<sha>`. `push` mode's
    /// fallback, and `charter save`'s recorded one; the request modes use a save branch instead.
    Branched,
    /// The branch requires a pull request and THAT push failed too.
    Stranded,
    /// An ordinary push failure, reported rather than diagnosed.
    Failed,
    /// The remote moved and the rebase onto it conflicted.
    Conflict,
    /// No origin on a forge charter knows — nothing to push to.
    Unreachable,
    /// A request mode pushed it to the save branch, and one pull request is open from there into
    /// the target branch (charter-app#298).
    PrOpen,
    /// A request mode cannot go further without a person: its pull request was closed without
    /// merging, the target branch no longer holds what was pushed, or somebody else pushed to
    /// this machine's save branch.
    Blocked,
}

impl Outcome {
    /// The word the record carries, which `doctor` reads back. Python's literals.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Pushed => "pushed",
            Outcome::Branched => "branched",
            Outcome::Stranded => "stranded",
            Outcome::Failed => "failed",
            Outcome::Conflict => "conflict",
            Outcome::Unreachable => "unreachable",
            Outcome::PrOpen => "pr-open",
            Outcome::Blocked => "blocked",
        }
    }
}

/// What [`push_head`] did, in a shape both a human and `doctor` can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushResult {
    pub outcome: Outcome,
    /// The branch charter tried to advance (the plane root's HEAD).
    pub branch: String,
    /// The remote branch the commit actually reached, when that is a different one. "It is on
    /// the remote under another name" and "it is on this laptop only" are one exit code apart
    /// and worlds apart in consequence.
    pub landed: Option<String>,
    pub url: Option<String>,
    /// The pull request's number, when a request mode opened or updated one.
    pub number: Option<u64>,
    pub detail: String,
    /// The files a rebase onto the remote conflicted in (charter-app#295).
    pub conflicts: Vec<String>,
    /// Why the push record of this outcome could not be written or cleared, where it could
    /// not: what purlis shows of the plane's saving may then not match it until the next
    /// save, and the save says so ([`said_if_unrecorded`], #1144).
    pub unrecorded: Option<String>,
}

impl PushResult {
    fn of(outcome: Outcome, branch: &str) -> PushResult {
        PushResult {
            outcome,
            branch: branch.to_string(),
            landed: None,
            url: None,
            number: None,
            detail: String::new(),
            conflicts: Vec::new(),
            unrecorded: None,
        }
    }
}

/// Says, as a warning, that the push record of `res` was not written, where it was not
/// ([`PushResult::unrecorded`]), and answers `res` as it was. What every push a save makes
/// ends with, so a reader of the save's output knows its standing may be stale.
pub(crate) fn said_if_unrecorded(res: PushResult, say: Sink) -> PushResult {
    if let Some(why) = &res.unrecorded {
        say(Say::Warn(format!(
            "This save's outcome was not recorded ({why}), so purlis may show the plane's \
             saving as it was before it until the next save."
        )));
    }
    res
}

// --------------------------------------------------------------------------------------- //
// the plane's origin                                                                        //
// --------------------------------------------------------------------------------------- //

/// The control plane's own `origin` as an HTTPS URL, rewriting an SSH one through ITS forge's
/// own rewrite rule — or `None` when there is no origin, when its host is not a forge this
/// charter knows, or when it carries userinfo.
///
/// `None` rather than a guess: the caller then warns and skips the push instead of silently
/// trying — and failing — against the wrong forge. Python's `_origin_https`.
///
/// **A remote carrying a credential is refused here, by construction.** A CI machine's
/// `https://x-access-token:<token>@github.com/acme/plane.git` does not start with the forge's
/// HTTPS base and is not one of its SSH forms, so it falls out of the rewrite below with
/// `None` — the token is neither pushed with nor named in the warning the caller then prints.
/// That is a property worth a test rather than a second check: the danger is not the `@`, it is
/// handing git a URL charter did not build.
///
/// # Testing this against a local stand-in forge, and the trap in doing so
///
/// **`git remote get-url` APPLIES `url.<base>.insteadOf`.** A test that stands a bare
/// repository up beside the plane and rewrites the forge's URL onto it — which is the only way
/// to exercise a push without reaching a real forge — decides, by which URL it keys the
/// rewrite, whether this function sees a forge at all:
///
/// - keyed on the HTTPS base (`url.file:///…/forge/acme/.insteadOf =
///   https://github.com/acme/`) with `origin` in the SSH form, `get-url` returns the SSH URL
///   untouched, [`forge::resolve_host`] places it, the rewrite below runs, and git maps the
///   HTTPS URL charter built onto the bare repository. This is the arrangement that tests
///   anything.
/// - keyed on the SSH form, or with `origin` already HTTPS, `get-url` hands back
///   `file:///…`. [`forge::resolve_host`] answers `None`, this function answers `None`, and
///   the caller warns and skips the push. **Both implementations agree perfectly and nothing
///   is pushed** — a green differential scenario over a code path neither side entered.
///
/// `env_clear()` in [`crate::worktree::git`] does not close this: `insteadOf` arrives through
/// CONFIG, not the environment, and `HOME` is the one variable that runner deliberately keeps
/// (its module docs say why), so `~/.gitconfig`'s rewrite reaches every git call charter
/// makes. It cannot be measured by looking at argv either — the rewrite happens inside git.
///
/// The differential therefore measured what `origin` resolved to before it ran the command,
/// and failed any scenario whose plane had a stand-in forge beside it and an origin that came
/// back `file://` without the scenario saying it meant that. Its scenarios are recorded now
/// (ADR 0046) and their setups cannot move; a NEW forge scenario has to make the same check by
/// hand: key `url.<local>.insteadOf` on the HTTPS base and leave `origin` in the SSH form.
pub fn origin_https(root: &Path) -> Option<String> {
    origin_https_of(root, root)
}

/// [`origin_https`] for a clone of a workspace: its own `origin`, placed on a forge the plane
/// at `plane` declares or a kind's default host.
pub fn origin_https_of(plane: &Path, root: &Path) -> Option<String> {
    let url = git::run(root, &["remote", "get-url", "origin"], git::READ)
        .ok()
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    if url.is_empty() {
        return None;
    }
    let forge = forge::resolve_host(&url, plane)?;
    forge::https_of(&url, &forge)
}

/// A one-click "open a pull request for this branch" URL, or `None`. Python's `_compare_url`.
///
/// A plain HTTPS link, deliberately: it closes the pull-request-gated workflow with no API call
/// and no extra token scope. The request modes open their PR through [`forge::pr`] instead, into an
/// explicit base (ADR 0051, #298); this link is left to `push` mode's protected-branch
/// fallback, where it is still resolved against the repo's default branch. Which form to
/// build is decided by RESOLVING the forge, never by
/// looking for a hostname inside the URL string: a self-hosted GitLab with a
/// `mirrors/github.com/…` namespace was handed GitHub's compare URL by the substring check
/// this replaces.
pub fn compare_url(https: &str, branch: &str, root: &Path) -> Option<String> {
    let base = https.strip_suffix(".git").unwrap_or(https);
    if !base.starts_with("https://") {
        return None;
    }
    let forge = forge::resolve_host(base, root)?;
    match forge.kind {
        forge::Kind::GitHub => Some(format!("{base}/compare/{branch}?expand=1")),
        // GitLab, and self-hosted GitLab, use the same new-MR form.
        forge::Kind::GitLab => Some(format!(
            "{base}/-/merge_requests/new?merge_request%5Bsource_branch%5D={branch}"
        )),
    }
}

// --------------------------------------------------------------------------------------- //
// the push record                                                                           //
// --------------------------------------------------------------------------------------- //

/// Where the push record lives. Public because `doctor` names it, which is what gives the
/// recorded `detail` — git's own words about a push nobody heard — a reader.
///
/// Under [`crate::plane::state_dir`] and not `<root>/.charter`, because `$CHARTER_HOME` moves
/// that directory verbatim: hardcoding the default wrote the record where `doctor` — which
/// already resolves it — would never look, and a record nobody reads is the same as no record.
pub fn push_record_path(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join("plane-push.json")
}

/// Write down what a push of the plane root's HEAD did, and return `res` unchanged.
///
/// A record and not a printed line, because the reactive-memory push runs detached with
/// `/dev/null` for a voice and has no caller to tell. What is written is the PAST tense — *a
/// push of commit `head` at time `at` came out this way* — which is the carve-out ADR 0011
/// makes: the present tense is reconstructed at read time by joining `head` against git
/// ([`is_spent`]), so a stale file cannot contradict a reality nobody re-derived.
///
/// A clean push DELETES the file rather than writing "pushed": the record exists to carry a
/// condition that outlives the process, and there is no condition left to carry.
///
/// Never fails loudly. Every caller is a push, and a push that cannot write a note must still
/// have pushed.
pub fn record_push(root: &Path, mut res: PushResult, head: &str) -> PushResult {
    let path = push_record_path(root);
    if res.outcome == Outcome::Pushed {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
            Err(why) => res.unrecorded = Some(why.to_string()),
        }
        return res;
    }
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_default();
    let mut doc = serde_json::json!({
        "outcome": res.outcome.word(),
        "branch": res.branch,
        "landed": res.landed,
        "url": res.url,
        "number": res.number,
        "detail": res.detail,
        "head": head,
        "at": at,
    });
    // Only when there are some, so a record without a conflict is the record it always was.
    if !res.conflicts.is_empty() {
        doc["conflicts"] = serde_json::json!(res.conflicts);
    }
    // NOT gated on `within_plane`: the state directory is machine-local and `$CHARTER_HOME`
    // may legitimately put it outside the plane, so that question is the wrong one to ask
    // about this file. `private_dir` refuses a state directory that is a symlink, and
    // `rewrite::replace` refuses a record that is one — so a link planted AT the record is
    // never written through.
    //
    // The state directory is what the containment walk is ROOTED at, for that same reason:
    // it is the deepest thing here charter already trusts, `private_dir` has just refused it
    // as a link, and the walk then covers the temp file the bytes actually land on
    // (charter-app#113).
    let written = match path.parent() {
        Some(dir) => crate::profiletrust::private_dir(dir).and_then(|()| {
            crate::rewrite::replace(
                dir,
                &path,
                crate::pyjson::dumps_indent2(&doc).as_bytes(),
                crate::rewrite::Mode::Private,
            )
        }),
        None => Err(std::io::Error::other("the record has no directory")),
    };
    if let Err(why) = written {
        res.unrecorded = Some(why.to_string());
    }
    res
}

/// The last recorded push outcome, or `None` when there is nothing to report.
///
/// `None` for a missing file AND for an unreadable or malformed one, deliberately: the only
/// consumer runs from a hook, where a defect in this file is charter's own to fix and not a
/// reason to take the session down.
///
/// # It is read the way `reopen` reads its record, and for the same reasons
///
/// This runs inside `charter save`, which an operator runs all day, and `read_to_string` on a
/// path is two hazards charter has already paid for once (charter-app #28):
///
/// - **A FIFO is not a link**, so a link check waves it through and reading one never
///   returns. There is no window to close and nothing to cancel — the save simply stops.
/// - **A planted giant** is read whole into memory. git cannot carry a FIFO, but a sparse
///   multi-gigabyte file packs small and arrives full size.
///
/// So the descriptor is opened with `O_NOFOLLOW | O_NONBLOCK` ([`crate::contain::open_no_link`])
/// and both questions are asked of THAT ([`crate::reopen::refuse_unusable`]), not of the name:
/// an `lstat` and a later open are two different objects with a window between them, and the
/// swap through that window is the failure the check exists to stop (ADR 0028).
///
/// **The walk starts at the state directory, not the plane root**, and that is the boundary
/// this file actually has: `$CHARTER_HOME` can put charter's machine-local state outside the
/// plane entirely, and "does any link leave the plane" is not a question about a path that was
/// never inside it. One component is left to walk — the record's own name — and that is
/// exactly the component a link or a FIFO can be.
pub fn push_record(root: &Path) -> Option<serde_json::Value> {
    let doc = read_json(&push_record_path(root))?;
    let has_outcome = doc
        .get("outcome")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|w| !w.is_empty());
    (doc.is_object() && has_outcome).then_some(doc)
}

/// A JSON file in the state directory, read as [`push_record`] reads its own: no link, no FIFO,
/// nothing bigger than a record, and `None` for anything unreadable.
fn read_json(path: &Path) -> Option<serde_json::Value> {
    let mut open = crate::contain::open_no_link(path.parent()?, path).ok()?;
    crate::reopen::refuse_unusable(path, &open.metadata().ok()?).ok()?;
    let text = {
        use std::io::Read;
        let mut text = String::new();
        // Bounded again on the way in: `refuse_unusable` asked how big it was, and a writer
        // that appends between the `fstat` and the read would otherwise still be unbounded.
        open.by_ref()
            .take(crate::reopen::MAX_BYTES)
            .read_to_string(&mut text)
            .ok()?;
        text
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .filter(serde_json::Value::is_object)
}

/// An object name as charter writes one: hex, and the length `rev-parse` answers with.
///
/// Both values this module takes out of `plane-push.json` end up in git's ARGV, and the file
/// is ordinary machine-local state that anything running as the operator can rewrite. A `head`
/// of `--help` makes `merge-base` exit 0 — which this module reads as "it landed" and a memory
/// is never pushed again. So each is held to the shape charter itself wrote rather than passed
/// through: git has no `--` that separates a revision from an option here, and a check is
/// cheaper than one that does.
fn is_object_name(text: &str) -> bool {
    (7..=64).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Has the commit a record is ABOUT since reached the tracked upstream?
///
/// `--is-ancestor` exits 0 for yes, 1 for no, and non-zero-not-1 for a ref it cannot resolve.
/// **Only a clean 0 counts**: "I could not check" must never read as "it landed", which is the
/// one place where getting that wrong loses a memory.
pub fn is_spent(root: &Path, head: &str) -> bool {
    if !is_object_name(head) {
        return false;
    }
    git::run(
        root,
        &["merge-base", "--is-ancestor", head, "@{upstream}"],
        git::READ,
    )
    .is_ok_and(|r| r.ok())
}

/// The recorded push outcome that is STILL true, or `None`. One decision, several renderings:
/// `doctor`'s row, the status line's one word, and [`land_via_branch`]'s "may I still advance
/// that branch".
pub fn unlanded(root: &Path) -> Option<serde_json::Value> {
    let rec = push_record(root)?;
    let head = rec.get("head").and_then(serde_json::Value::as_str)?;
    if prsave::is_pr_record(&rec) {
        return prsave::still_holds(root, &rec, head).then_some(rec);
    }
    (!is_spent(root, head)).then_some(rec)
}

/// What the Saving view and the title bar say of a push record that calls itself a conflict
/// and names nothing (qw382): a build before qw348 wrote every failed rebase that way, so the
/// word is not believed, and the next save retries the push.
pub const STOPPED_EARLIER: &str = "Save stopped earlier — Save to retry";

/// Whether a push record says `conflict` and names neither a reason nor a file: a record no
/// conflict is believed of (qw382). A conflict this build records always names its files.
fn names_no_conflict(rec: &serde_json::Value) -> bool {
    let empty = |key: &str| match rec.get(key) {
        None | Some(serde_json::Value::Null) => true,
        Some(serde_json::Value::String(text)) => text.trim().is_empty(),
        Some(serde_json::Value::Array(all)) => all.is_empty(),
        Some(_) => false,
    };
    rec.get("outcome").and_then(serde_json::Value::as_str) == Some(Outcome::Conflict.word())
        && empty("detail")
        && empty("conflicts")
}

/// Whether the next save of a clean plane pushes again because its last push stopped and has
/// not landed since (qw382): a push-path record of a conflict or a stranded commit. A Save
/// pressed on such a plane re-runs the push and its retry even where it names no mode, so the
/// answer it leaves is a current one: the push lands and the record goes, or it is written
/// again in today's words.
fn retries_its_push(root: &Path) -> bool {
    unlanded(root).is_some_and(|rec| {
        !prsave::is_pr_record(&rec)
            && matches!(
                rec.get("outcome").and_then(serde_json::Value::as_str),
                Some("conflict" | "stranded")
            )
    })
}

/// Forget a push-path record once the project's own commits were replayed onto the remote's
/// by the window's Update (qw382): the record is about a commit the replay rewrote, so it can
/// never land, and the rebase it says stopped has since gone through. A request mode's record
/// is its request's, and is left to it.
pub(crate) fn forget_replayed_push(root: &Path) {
    let Some(rec) = push_record(root) else {
        return;
    };
    if prsave::is_pr_record(&rec) {
        return;
    }
    let _ = std::fs::remove_file(push_record_path(root));
    touch(root);
}

// --------------------------------------------------------------------------------------- //
// where unsaved work sits                                                                   //
// --------------------------------------------------------------------------------------- //

/// Where a plane's unsaved work sits, furthest back first (ADR 0051). The Saving view and the
/// title bar show the furthest-back one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// A save cannot go further without a person: the last push conflicted or failed, and
    /// its commit has not landed since.
    Blocked,
    /// Files the next save would commit.
    Changed,
    /// Commits the remote does not have yet.
    Committed,
    /// Pushed, and waiting on a pull request.
    PrOpen,
    /// On the target branch.
    Saved,
}

impl Stage {
    pub fn word(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Changed => "changed",
            Self::Committed => "committed",
            Self::PrOpen => "pr-open",
            Self::Saved => "saved",
        }
    }
}

/// A plane's save standing, read from git and the push record — never from the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub stage: Stage,
    /// What the next save would commit: tracked changes and new files git does not ignore,
    /// as `git add -A` would take them, sorted.
    pub changed: Vec<String>,
    /// Commits on HEAD that the remote-tracking branch does not have. `None` when there is no
    /// remote-tracking branch to count against.
    pub ahead: Option<u32>,
    /// The pull request a pushed commit is waiting on.
    pub pr: Option<String>,
    /// Why the save is blocked, in git's own words.
    pub blocked: Option<String>,
    /// The target branch commits are counted against: `[plane] branch`, or the one HEAD is on.
    pub branch: String,
    /// Whether a save would push: a mode that goes past the commit, and an origin on a forge
    /// charter knows. When it would not, a commit is as far as a save goes, and a plane with
    /// nothing left to commit is saved.
    pub pushes: bool,
    /// Commits the remote-tracking branch has that HEAD does not: what the last fetch brought
    /// in and has not been fast-forwarded to. `None` when there is nothing to count against.
    pub behind: Option<u32>,
    /// Why the last push did not land, when it failed rather than conflicted — offline, a
    /// token, the forge down. Not blocked: auto-save tries again later.
    pub push_failed: Option<String>,
    /// The files to settle, when conflicts are why it is blocked: those a merge or rebase
    /// stopped part-way left unmerged, else those the last save's rebase conflicted in.
    pub conflicts: Vec<String>,
    /// What a save cannot do here that is not a block — a request mode on a remote no forge adapter
    /// serves, where a save still commits and goes no further.
    pub notice: Option<String>,
    /// Whether a file git tracks is among [`Standing::changed`]: the plane root being worked
    /// in, as the alerts say (`alerts::tracked_dirty`'s rule, read from this same `status` in
    /// the app, FD-11). New files alone are not.
    pub tracked: bool,
    /// HEAD's commit as it was read, or empty when there is none: what auto-save's
    /// [`fingerprint_of`] starts from, so a look asks git for it once.
    pub head: String,
    /// Each changed file the next save would refuse for what looks like a credential, while
    /// that is why it is blocked ([`SECRET_REFUSED`]); empty otherwise.
    pub secrets: Vec<SecretAt>,
    /// Whether the save is blocked by the push record of a save's replay onto the remote that
    /// conflicted, git not being stopped part-way now (qw382): the person settles it with a
    /// rebase of their own, since a chat may not rebase the plane root.
    pub rebase_by_hand: bool,
    /// What git is stopped part-way in, by kind (qw382): an operation's
    /// [`gitstate::Operation::kind`], or `conflicts` for unmerged files and no operation.
    /// `None` when it is not. A window decides by it which ways out it offers: a chat may
    /// finish a merge, and may not continue the others in the plane root.
    pub stopped_by: Option<&'static str>,
    /// Where the open pull request of a `pr-merge` plane stands with merging, as the last look
    /// at the forge found it: waiting on its checks, red with the checks that failed, in a
    /// merge queue, unable to merge, or merged and not moved onto yet. `None` when no request
    /// is open, or the mode does not merge it. Never a block: a red request stays open, and a
    /// later save pushes onto it (ADR 0051, amended 2026-10-13).
    pub merge: Option<Merging>,
}

impl Standing {
    /// Why the save is blocked, as a window says it: [`Standing::blocked`], and for a
    /// secret-shaped value the file and line it is in ([`blockedby::said`]). `blocked` itself
    /// stays the words the journal and every reader match whole.
    pub fn blocked_said(&self) -> Option<String> {
        blockedby::said(&self.secrets).or_else(|| self.blocked.clone())
    }
}

/// Where the plane at `root`'s unsaved work sits.
pub fn standing(root: &Path) -> Standing {
    let plane = crate::planesave::Settings::read(root).plane;
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string());
    let Some(here) = here else {
        return Standing {
            stage: Stage::Blocked,
            changed: Vec::new(),
            ahead: None,
            pr: None,
            blocked: Some(
                "this plane is not a git repository, so there is nothing to commit to".into(),
            ),
            branch: String::new(),
            pushes: false,
            behind: None,
            push_failed: None,
            conflicts: Vec::new(),
            notice: None,
            secrets: Vec::new(),
            rebase_by_hand: false,
            stopped_by: None,
            merge: None,
            // An unborn branch is a repository with nothing to count against, and what is
            // staged in it is still the root being worked in (FD-11: the alerts read this).
            tracked: status_of(root).1,
            head: String::new(),
        };
    };
    let here_branch = here.clone();
    let head = git::run(root, &["rev-parse", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let branch = plane.branch.value.clone().unwrap_or(here);
    // A request mode whose settings cannot work on a forge charter knows — a save_branch that is
    // the target branch itself — is blocked until they are fixed, read from the settings as
    // they are now (charter-app#298). A request mode on a remote no forge adapter serves is the
    // notice below, not a block.
    let misconfigured = plane
        .mode
        .value
        .filter(|mode| mode.opens_a_pr() && origin_https(root).is_some())
        .and_then(|_| prsave::config(root, &plane).err());
    let pushes = matches!(
        plane.mode.value,
        None | Some(
            crate::planesave::Mode::Push
                | crate::planesave::Mode::Pr
                | crate::planesave::Mode::PrMerge
        )
    ) && misconfigured.is_none()
        && origin_https(root).is_some();
    let (changed, tracked) = status_of(root);
    let ahead = count(root, &format!("refs/remotes/origin/{branch}..HEAD"));
    let behind = count(root, &format!("HEAD..refs/remotes/origin/{branch}"));
    let record = unlanded(root);
    let said = |key: &str| {
        record
            .as_ref()
            .and_then(|r| r.get(key))
            .and_then(serde_json::Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    // A record that calls itself a conflict and names nothing is not believed (qw382): it is a
    // save that stopped, which the next save retries, and never a block.
    let unnamed = record.as_ref().is_some_and(names_no_conflict);
    let outcome = said("outcome").filter(|_| !unnamed);
    let pr = matches!(outcome.as_deref(), Some("branched" | "pr-open"))
        .then(|| said("url"))
        .flatten()
        .or_else(|| {
            // A request mode's open PR stays known when a later save failed (charter-app#298).
            plane
                .mode
                .value
                .is_some_and(crate::planesave::Mode::opens_a_pr)
                .then(|| prsave::known_pr_url(root))
                .flatten()
        });
    let push_failed = (outcome.as_deref() == Some(Outcome::Failed.word()))
        .then(|| said("detail").unwrap_or_else(|| "the push failed".into()))
        .or_else(|| unnamed.then(|| STOPPED_EARLIER.to_owned()));
    let conflicts: Vec<String> = record
        .as_ref()
        .and_then(|r| r.get("conflicts"))
        .and_then(serde_json::Value::as_array)
        .map(|all| {
            all.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let notice = plane
        .mode
        .value
        .filter(|mode| mode.opens_a_pr() && origin_https(root).is_none())
        .map(|mode| {
            format!(
                "[plane] mode is {}, and this plane's origin is not a GitHub or GitLab forge \
                 purlis knows, so a save goes no further than a commit",
                mode.as_str()
            )
        });
    // Every block is read from what is true now, never kept as a note: a block resolved by
    // hand — a rebase finished in a terminal, a secret removed, the right branch checked out —
    // is gone the moment it is, because auto-save waits while a plane is blocked and would
    // never write the line that cleared a stored one.
    // A request mode's `blocked` record (charter-app#298) holds only while the plane is still on the
    // commit it is about and that commit is not on the target (`unlanded`): moving the plane
    // off it by hand, or a save that opens a new pull request, clears it.
    let mut secrets: Vec<SecretAt> = Vec::new();
    let blocked = matches!(
        outcome.as_deref(),
        Some("conflict" | "stranded" | "blocked")
    )
    .then(|| said("detail").unwrap_or_else(|| outcome.clone().unwrap_or_default()))
    .or(misconfigured)
    // The plane on another branch than the one it saves into: a save commits and does not
    // push (#293), and that is the operator's to settle.
    .or_else(|| {
        plane
            .branch
            .value
            .as_deref()
            .filter(|target| *target != here_branch)
            .map(|target| format!("this plane is on {here_branch}, and [plane] branch is {target}"))
    })
    // A secret the next save would refuse: a changed file that holds one now. Asked of the
    // files as they are, so removing it clears the block at once. Each is kept, so a window
    // says which file and line (`blockedby`).
    .or_else(|| {
        secrets = blockedby::among(root, &changed);
        (!secrets.is_empty()).then(|| SECRET_REFUSED.to_owned())
    });
    // Git stopped part-way through a merge or rebase, asked of the tree now (#433): named
    // before any other block, since a save refuses it first and auto-save waits while it is.
    let stopped = gitstate::stopped(root);
    // The record's conflict is the block only while git is not stopped part-way, which is
    // named first and settled by its own way out.
    let rebase_by_hand = stopped.is_none()
        && blocked.is_some()
        && outcome.as_deref() == Some(Outcome::Conflict.word());
    let stopped_by = stopped.as_ref().map(|stopped| {
        stopped
            .operation
            .map_or("conflicts", gitstate::Operation::kind)
    });
    let blocked = stopped.as_ref().map(gitstate::Stopped::why).or(blocked);
    // Named only while they are why: a stopped merge is settled first.
    if blocked.as_deref() != Some(SECRET_REFUSED) {
        secrets.clear();
    }
    // A request mode's pull request carries the commit it was last pushed at; a commit made since
    // is not in it yet.
    let waiting = match outcome.as_deref() {
        Some("branched") => pr.is_some(),
        Some("pr-open") => said("head").as_deref() == Some(head.as_str()),
        _ => false,
    };
    let stage = if blocked.is_some() {
        Stage::Blocked
    } else if !changed.is_empty() {
        Stage::Changed
    } else if waiting {
        Stage::PrOpen
    } else if pushes && ahead != Some(0) {
        // Nothing to count against is not nothing unpushed: say committed, never saved.
        Stage::Committed
    } else {
        Stage::Saved
    };
    // Only while a request is known to be open: what is kept goes when it is settled.
    let merge = (plane.mode.value == Some(crate::planesave::Mode::PrMerge) && pr.is_some())
        .then(|| prsave::known_merging(root))
        .flatten();
    Standing {
        stage,
        changed,
        ahead,
        pr,
        blocked,
        branch,
        pushes,
        behind,
        push_failed,
        // The files a stopped merge or rebase left unmerged are the ones to settle now.
        conflicts: match stopped {
            Some(stopped) if !stopped.unmerged.is_empty() => stopped.unmerged,
            _ => conflicts,
        },
        notice,
        tracked,
        head,
        secrets,
        rebase_by_hand,
        stopped_by,
        merge,
    }
}

/// Where a look at a save's open request is run from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looker {
    /// The app, for the person whose window it is.
    Window,
    /// Anything else, with nobody there.
    Unattended,
}

/// **Who the app's own look asks the forge as.** The one place it is chosen, for the plane's
/// look and every repo's.
///
/// From the window it asks as a save pressed there would, with nobody waiting on it: with the
/// person's purlis sign-in where there is one for the forge, and through the forge's own CLI
/// otherwise (ADR 0070 §5, amended 2026-10-14 by the operator's ruling, for this one job).
/// Anywhere else it is a trigger, which never takes the person's sign-in.
///
/// Nothing else that runs with nobody at the keyboard asks as the window: an automatic save's
/// own calls stay a trigger's ([`Trigger::caller`]).
pub fn look_caller(from: Looker) -> forge::Caller {
    match from {
        Looker::Window => forge::Caller::window().background(),
        Looker::Unattended => forge::Caller::trigger(),
    }
}

/// The commit the project's open request is at and where it stands, as the clone keeps
/// them: what a confirmation of the person's merge shows, and hands back ([`merge_as_seen`]).
pub fn seen(root: &Path) -> Option<Seen> {
    let (head, merge) = prsave::kept_seen(root)?;
    Some(Seen {
        head,
        shown: merge?.shown(),
    })
}

/// **The person's merge, of what they were shown** (the operator's ruling P1): the request is
/// looked at afresh first, with every check read, and merged as it stands only when it is at
/// the commit and in the standing the confirmation showed ([`changed_since`]). A check that
/// failed since, or a commit pushed since, is never merged past: the refusal says it changed,
/// and what the look found is kept for the window to draw. A request the look itself merged,
/// because every check on it passed, is merged and nothing more is asked.
pub fn merge_as_seen(
    root: &Path,
    caller: &forge::Caller,
    seen: &Seen,
    say: Sink,
) -> Result<MergedAsItStands, String> {
    let looked = look(root, caller, say)?;
    touch(root);
    let Some((head, now)) = prsave::kept_seen(root) else {
        return Ok(if looked.moved {
            MergedAsItStands::Moved
        } else {
            MergedAsItStands::Later(
                "Every check on it passed, and purlis merged it as its checks stand.".into(),
            )
        });
    };
    if let Some(changed) = changed_since(seen, &head, now.as_ref()) {
        return Err(changed);
    }
    merge_as_it_stands(root, caller, seen, say)
}

/// **What the window's Fix hands a chat about the project's red pull request** (the operator's
/// ruling P2, 2026-10-13): the request's number, and what failed on it with what each failed
/// check found, asked of the forge as `caller`. Refused unless the last look found it red.
/// Nothing is written and nothing is merged.
pub fn trouble(
    root: &Path,
    caller: &forge::Caller,
) -> Result<(u64, crate::savefix::Trouble), String> {
    let plane = crate::planesave::Settings::read(root).plane;
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let branch = plane.branch.value.clone().unwrap_or(here);
    prsave::trouble(root, &plane, &branch, caller)
}

/// What a look at a `pr-merge` plane's open pull request came to ([`look`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Looked {
    /// A request is still open after the look, so there is something to look at again.
    pub open: bool,
    /// What the standing says changed: the request merged, or its state did.
    pub changed: bool,
    /// The plane was moved onto what merged, so its working tree moved.
    pub moved: bool,
}

/// The request this plane's saves keep open: when its head was pushed (seconds since the
/// epoch; `None` when not kept) and its head. `None` when nothing is open. One small file, and
/// no git: what the app paces its looks by ([`crate::autosave::Looks`]). It does not ask the
/// mode, which [`look`] does: a caller that has the settings asks only for a `pr-merge` plane.
pub fn request_to_look_at(root: &Path) -> Option<(Option<u64>, String)> {
    prsave::open_request(root)
}

/// **Look again at a `pr-merge` plane's open pull request**, with no save: merge it if every
/// check has passed since, or if it has been there long enough and nothing says a check will
/// run, and then move the plane onto what merged; otherwise keep where it stands (waiting,
/// red with the failed checks, unable to merge) for the standing to say. This is what merges a
/// green request without the person saving again (ADR 0051, amended 2026-10-13).
///
/// One look is a handful of forge reads. The caller paces them (the app's auto-save worker
/// backs off from a minute to five, and stops after two hours; `app/src-tauri/src/autosave.rs`).
/// `Err` when a save of the plane is running, or the target could not be fetched from.
pub fn look(root: &Path, caller: &forge::Caller, say: Sink) -> Result<Looked, String> {
    let _claim = Claim::of(root).ok_or(ALREADY_SAVING)?;
    let plane = crate::planesave::Settings::read(root).plane;
    if plane.mode.value != Some(crate::planesave::Mode::PrMerge) {
        return Ok(Looked::default());
    }
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    if here != branch {
        return Ok(Looked::default());
    }
    let before = prsave::known_merging(root);
    let fetch = prsave::fetch_for_a_look(root, &plane)?;
    let fetch = fetch
        .as_ref()
        .map(|(https, helper)| (https.as_str(), helper.as_str()));
    let settled = prsave::settle(root, &plane, &branch, plane.sign.value, fetch, caller, say);
    // What is kept beside the request moved, and no git file did: the shared standing is told.
    touch(root);
    let moved = settled == prsave::Settled::Moved;
    let after = prsave::known_merging(root);
    Ok(Looked {
        open: prsave::knows_a_pr(root),
        changed: moved || before != after || matches!(settled, prsave::Settled::Blocked(_)),
        moved,
    })
}

/// **Merge the plane's open save request as it stands**, its failed and missing checks
/// included. The person's own action in the window, and only there: no save, no look and no
/// chat ever calls this (the operator's ruling P1, 2026-10-13). Every other rule still holds: a
/// conflict, a review the branch requires and whatever the forge itself enforces refuse it.
///
/// Private, and reached only from [`merge_as_seen`] (#1978): the press that showed the
/// request's checks. `seen` is what that press showed, and it is checked again under this
/// merge's own claim: a request at another commit by now, or one whose standing changed since
/// (its checks failed after the look), is refused.
fn merge_as_it_stands(
    root: &Path,
    caller: &forge::Caller,
    seen: &Seen,
    say: Sink,
) -> Result<MergedAsItStands, String> {
    let _claim = Claim::of(root).ok_or(ALREADY_SAVING)?;
    let plane = crate::planesave::Settings::read(root).plane;
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    if here != branch {
        return Err(format!(
            "this project is on {here}, and its saves go into {branch}"
        ));
    }
    let merged =
        prsave::merge_as_it_stands(root, &plane, &branch, plane.sign.value, seen, caller, say);
    touch(root);
    merged
}

/// The longest a shared plane standing is kept without its git files moving or anyone saying
/// the plane changed: the backstop for an edit no watcher reported, which is how stale the
/// title bar could already be between its reads.
pub const SHARED_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// Every plane's standing, one per plane.
static PLANES: std::sync::LazyLock<crate::standings::Shared<Standing>> =
    std::sync::LazyLock::new(|| crate::standings::Shared::new(SHARED_FOR));

/// [`standing`], **shared by every caller in this process** (FD-11, #651): auto-save's look, the
/// title bar, the Saving tab and the alerts ask this, and git runs once for all of them —
/// one computation at a time per plane, again only when the plane's git files move
/// ([`crate::standings::Stamp`]), [`touch`] says it changed, or [`SHARED_FOR`] has passed.
///
/// A save's own decisions read [`standing`] directly, as they always did: what is committed is
/// decided from the tree as it is at that moment, never from a shared answer.
pub fn shared_standing(root: &Path) -> Standing {
    PLANES.get(root, || standing(root))
}

/// Something at or under `dir` changed — a plane its watcher saw move, a plane or a clone a save
/// or a fetch let go of, a setting written: the next shared standing of every plane and every
/// clone there ([`shared_standing`], [`crate::reposave::shared_standing`]) reads git again. The
/// one call for both stores, since a plane's settings are what each of its clones reads too.
pub fn touch(dir: &Path) {
    PLANES.touch_within(dir);
    crate::reposave::touch_within(dir);
}

/// `git rev-list --count <range>`, or `None` when git cannot count it — a ref that is not there.
pub(crate) fn count(root: &Path, range: &str) -> Option<u32> {
    git::run(root, &["rev-list", "--count", range], git::READ)
        .ok()
        .filter(git::Run::ok)
        .and_then(|r| r.line().trim().parse().ok())
}

/// What is unsaved in the plane, as one string that changes whenever it does: HEAD, the
/// commits the remote lacks, and each changed file with its size and modification time — so a
/// file written again is a change even when git's one-letter status for it is not.
///
/// Auto-save's quiet period is measured from the last time this changed
/// ([`crate::autosave::Quiet`]).
pub fn fingerprint(root: &Path) -> String {
    fingerprint_of(root, &standing(root))
}

/// [`fingerprint`], from a [`Standing`] already read — so a look at the plane asks git for its
/// status once, not twice.
pub fn fingerprint_of(root: &Path, standing: &Standing) -> String {
    let mut out = format!("{}\0{:?}", standing.head, standing.ahead);
    for path in &standing.changed {
        let seen = std::fs::symlink_metadata(root.join(path)).ok().map(|meta| {
            let at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (meta.len(), at)
        });
        out.push('\0');
        out.push_str(path);
        out.push_str(&format!("{seen:?}"));
    }
    out
}

/// What a fetch of the plane's target branch found, and whether the plane was moved onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    /// Commits the fetch found on the remote that the plane did not have.
    pub behind: u32,
    /// Whether the plane was fast-forwarded onto them.
    pub moved: bool,
    /// Why commits that came in were left on the remote-tracking branch: `Some` only when a
    /// fast-forward was asked for, something came in, and the plane was not moved.
    pub held: Option<Held>,
}

/// Why a fetch left incoming commits where they are rather than moving the plane onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// A merge, rebase or bisect stopped part-way, or files git still calls unmerged. First:
    /// it is the one a person has to settle, and a save now would commit the markers.
    Stopped(gitstate::Stopped),
    /// Another git command holds the index, or one died holding it.
    IndexLocked,
    /// The plane is checked out on a branch that is not its target branch.
    OffTarget { here: String, branch: String },
    /// The plane has commits of its own the remote lacks, so a fast-forward cannot reach.
    Ahead(u32),
    /// Changed files nobody has saved yet.
    Unsaved,
    /// `git merge --ff-only` itself refused, in git's words.
    Refused(String),
    /// This process runs under a chat's sandbox, which denies it writing these files the
    /// incoming commits change: the move would stop partway (qw346).
    Sandbox(Vec<String>),
}

impl Held {
    /// The reason, as a clause: "N incoming commits left on origin/main: <this>".
    pub fn why(&self) -> String {
        match self {
            Self::Stopped(stopped) => stopped.why(),
            Self::IndexLocked => "git's index is locked by another git command".into(),
            Self::OffTarget { here, branch } => {
                format!("the plane is on `{here}`, not its target branch `{branch}`")
            }
            Self::Ahead(n) => format!(
                "the plane has {} of its own the remote lacks, and a pull only \
                 fast-forwards",
                counted(*n, "commit", "commits")
            ),
            Self::Unsaved => "the tree has unsaved changes, and a pull never moves them".into(),
            Self::Refused(detail) => format!("git refused the fast-forward: {detail}"),
            Self::Sandbox(files) => format!(
                "{}; the window's Update brings them in, or the person runs `purlis save \
                 --pull` in their own terminal",
                said_sandbox_stops(files)
            ),
        }
    }
}

/// Fetch the plane's target branch, and — when `fast_forward` (auto-save is on, or `charter
/// save --pull` asked) — move onto it when that cannot touch anyone's work (charter-app#296): a
/// clean tree, on the target branch, with nothing of its own the remote lacks, and no merge or
/// rebase in progress. Otherwise the plane is left exactly as it is, [`Incoming::held`] says
/// why, and the next save's rebase brings the commits in.
///
/// `Err` when there is nothing to fetch from — no origin on a forge charter knows — or the
/// fetch failed, in git's words.
pub fn fetch(root: &Path, fast_forward: bool) -> Result<Incoming, String> {
    // A fetch moves refs and may move the tree: never while a save of the plane runs.
    let _claim = Claim::of(root).ok_or(ALREADY_SAVING)?;
    let plane = crate::planesave::Settings::read(root).plane;
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    fetch_branch(root, &branch)?;
    // A pull request a request mode opened may have merged, or been closed, since the last look:
    // settled before the fast-forward below, which a merged PR's squash would never allow.
    let behind = count(root, &format!("HEAD..refs/remotes/origin/{branch}")).unwrap_or(0);
    let settled = fast_forward
        && here == branch
        && prsave::settle(
            root,
            &plane,
            &branch,
            plane.sign.value,
            None,
            &forge::Caller::command(),
            &mut |_| {},
        ) == prsave::Settled::Moved;
    let mine = count(root, &format!("refs/remotes/origin/{branch}..HEAD")).unwrap_or(0);
    let git_dir = git_dir_of(root);
    let held = (fast_forward && !settled && behind > 0)
        .then(|| hold(root, &git_dir, &here, &branch, mine))
        .flatten();
    let (moved, held) = if settled {
        (true, None)
    } else if fast_forward && behind > 0 && held.is_none() {
        // Untimed: a merge checks out a tree, and a killed one leaves it half-written.
        match git::run_untimed(
            root,
            &[
                "merge",
                "--ff-only",
                "--no-overwrite-ignore",
                &format!("refs/remotes/origin/{branch}"),
            ],
        ) {
            Ok(run) if run.ok() => (true, None),
            Ok(run) => (false, Some(Held::Refused(tail(&run)))),
            Err(unavailable) => (false, Some(Held::Refused(unavailable.to_string()))),
        }
    } else {
        (false, held)
    };
    Ok(Incoming {
        behind,
        moved,
        held,
    })
}

/// The network half of [`fetch`], and of the window's Update ([`update::update`]): `branch` of
/// the plane's origin, fetched into its remote-tracking ref and nothing else. One function, so
/// both are held to the same remote: the URL is one purlis built from the origin it reads, the
/// call goes through the runner's network entry point, and the window's own sign-in goes only
/// to the remote recorded for the plane, as its push does (#1851).
fn fetch_branch(root: &Path, branch: &str) -> Result<(), String> {
    let https = origin_https(root).ok_or("origin is not on a forge purlis knows")?;
    let forge = crate::gitpolicy::forge_for(root, root)
        .unwrap_or_else(|| Forge::default_of(forge::DEFAULT_KIND));
    let helper = forge::gitcredential::helper_for_fetch(&forge, root).map_err(str::to_string)?;
    let fetched = git::run_network(
        root,
        Some(helper.as_str()),
        &[
            "fetch",
            "--no-recurse-submodules",
            // A save's rebase reads FETCH_HEAD; a fetch of charter's own leaves it alone.
            "--no-write-fetch-head",
            &https,
            &format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"),
        ],
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if !fetched.ok() {
        return Err(tail(&fetched));
    }
    keep_fetched_upstream(root, &https, &helper, branch);
    Ok(())
}

/// **Keeps the upstream commit the remote names for `branch`** (#2156), in this machine's
/// config home, where no chat writes ([`crate::sandbox::local::keep_fetched_upstream`]): what
/// the project's sandbox off is checked against before its ask names a commit and an author.
/// The remote's own answer, asked after the fetch, never a ref in the clone, and kept only
/// where the fetch brought that commit here. Best effort: where it cannot be asked or kept, the
/// one kept before stands.
fn keep_fetched_upstream(root: &Path, https: &str, helper: &str, branch: &str) {
    let name = format!("refs/heads/{branch}");
    let Ok(listed) = git::run_network(root, Some(helper), &["ls-remote", https, &name]) else {
        return;
    };
    if !listed.ok() {
        return;
    }
    let Some(commit) = listed.out.lines().find_map(|line| {
        let (commit, at) = line.split_once('\t')?;
        (at.trim() == name && is_commit_id(commit)).then(|| commit.to_owned())
    }) else {
        return;
    };
    let here = git::run(
        root,
        &[
            "--no-replace-objects",
            "cat-file",
            "-e",
            &format!("{commit}^{{commit}}"),
        ],
        git::READ,
    );
    if here.is_ok_and(|run| run.ok()) {
        let _ = crate::sandbox::local::keep_fetched_upstream(root, &commit);
    }
}

/// Whether `said` is a commit's full id as git prints one.
fn is_commit_id(said: &str) -> bool {
    matches!(said.len(), 40 | 64) && said.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Why a fast-forward onto incoming commits would touch someone's work, or `None` when it
/// cannot (charter-app#296): a clean tree, on the target branch, with nothing of its own the
/// remote lacks, and no merge or rebase in progress.
fn hold(root: &Path, git_dir: &Path, here: &str, branch: &str, mine: u32) -> Option<Held> {
    if let Some(stopped) = gitstate::stopped(root) {
        return Some(Held::Stopped(stopped));
    }
    if crate::gitstate::find(git_dir).is_some() {
        return Some(Held::IndexLocked);
    }
    if here != branch {
        return Some(Held::OffTarget {
            here: here.to_string(),
            branch: branch.to_string(),
        });
    }
    if mine > 0 {
        return Some(Held::Ahead(mine));
    }
    if !changed_paths(root).is_empty() {
        return Some(Held::Unsaved);
    }
    let stops = sandbox_would_stop(root, &format!("refs/remotes/origin/{branch}"));
    if !stops.is_empty() {
        return Some(Held::Sandbox(stops));
    }
    None
}

/// The files a move of the tree at `root` from HEAD to `target` would write that the sandbox
/// this process runs under denies it, or none: outside a chat's sandbox, where the two trees
/// are the same, and where git cannot say how they differ (the move then fails on its own,
/// before a file is written).
///
/// **A move that cannot finish does not start** (qw346). git checks a tree out file by file and
/// skips what it may not write, so a fast-forward, a rebase or a reset run by `purlis save`
/// inside a chat's sandbox left the project half-updated wherever the other commit changed the
/// manifest or a later-code file, with nothing to say an update was tried. The window's own
/// save runs under no sandbox and is never held here.
pub(crate) fn sandbox_would_stop(root: &Path, target: &str) -> Vec<String> {
    if !crate::sandbox::held::in_a_sandbox(root) {
        return Vec::new();
    }
    // `diff-tree`, the plumbing: two trees compared, the index neither read nor refreshed.
    let changed: Vec<String> = git::run(
        root,
        &[
            "diff-tree",
            "-r",
            "--name-only",
            "--no-renames",
            "-z",
            "HEAD",
            target,
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .map(|run| {
        run.out
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default();
    crate::sandbox::held::checkout_held(root, &changed)
}

/// Why a move of the tree is not made under a chat's sandbox, as a clause naming the files.
pub(crate) fn said_sandbox_stops(files: &[String]) -> String {
    let shown: Vec<String> = files.iter().take(4).map(|f| format!("`{f}`")).collect();
    let more = files.len().saturating_sub(shown.len());
    let and_more = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    format!(
        "this chat's sandbox does not let it write {}{and_more}, which the remote's commits \
         change, so bringing them in from here would stop partway and leave the project \
         half-updated",
        shown.join(", ")
    )
}

/// The repository's own directory, which is a file's target for a worktree plane.
fn git_dir_of(root: &Path) -> PathBuf {
    git::run(root, &["rev-parse", "--absolute-git-dir"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map_or_else(|| root.join(".git"), |r| PathBuf::from(r.line().trim()))
}

/// `charter save --pull`'s first half: [`fetch`] the plane's target branch and fast-forward
/// onto it, exactly as the app's incoming loop does with auto-save on (ADR 0051), and say
/// what happened. The pull is asked for, so it is made whatever the plane's mode — `off`
/// included, where the save that follows then commits nothing.
///
/// `0` when there is nothing to stop the save that follows: nothing came in, it was brought
/// in, or it was left for a reason the save does not make worse (unsaved work, which the save
/// commits). `1` when the fetch failed, or the tree has conflicts — saving then would stage
/// the conflict markers, so the caller stops.
pub fn pull(root: &Path, say: Sink) -> u8 {
    let incoming = match fetch(root, true) {
        Ok(incoming) => incoming,
        Err(why) => {
            say(Say::Fail(format!("Could not pull the plane: {why}")));
            return 1;
        }
    };
    let n = incoming.behind;
    let commits = if n == 1 { "commit" } else { "commits" };
    // Conflicts stop the save even when nothing came in: a save would stage their markers.
    let held = incoming
        .held
        .or_else(|| gitstate::stopped(root).map(Held::Stopped));
    match (&held, incoming.moved) {
        (_, true) => {
            say(Say::Done(format!("Brought in {n} incoming {commits}.")));
            0
        }
        (None, false) => {
            say(Say::Info("Nothing incoming.".into()));
            0
        }
        (Some(held), false) => {
            let line = if n == 0 {
                format!("Nothing incoming, but {}.", held.why())
            } else {
                format!("{n} incoming {commits} not brought in: {}.", held.why())
            };
            if matches!(held, Held::Stopped(_)) {
                say(Say::Fail(line));
                say(Say::Info(
                    "  Nothing was saved. Settle that, then run `purlis save --pull` again.".into(),
                ));
                1
            } else {
                say(Say::Warn(line));
                0
            }
        }
    }
}

/// `git status --porcelain=v1 -z`'s paths: what `git add -A` would take. `-z` for the same
/// reason the save's secret guard uses it — no quoting, NUL the only separator.
pub(crate) fn changed_paths(root: &Path) -> Vec<String> {
    status_of(root).0
}

/// [`changed_paths`], and whether any of them is a file git tracks (any entry but `??`).
fn status_of(root: &Path) -> (Vec<String>, bool) {
    // What a save stages ([`save_stages`]): never what is [`never_staged`].
    let mut args = vec![
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
        ".",
    ];
    let excluded = never_staged();
    args.extend(excluded.iter().map(String::as_str));
    let Ok(run) = git::run(
        root,
        // `--no-optional-locks`, as `profiles.rs` and `guest.rs` ask: a plain `status`
        // refreshes the index and takes `index.lock`, and the title bar asks this every ten
        // seconds — a save's `git add -A` landing inside that window failed on the lock.
        // No `-c core.untrackedCache`: a read-only status uses a cache the index holds without
        // it, and git never consults one under `--untracked-files=all` (FD-11, measured).
        &args,
        git::READ,
    ) else {
        return (Vec::new(), false);
    };
    let mut out = Vec::new();
    let mut tracked = false;
    let mut entries = run.out.split('\0');
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let (status, path) = entry.split_at(3);
        tracked |= !status.starts_with("??");
        out.push(path.to_string());
        // A rename or copy names where it came from next, as an entry of its own.
        if status.contains(['R', 'C']) {
            entries.next();
        }
    }
    out.sort();
    (out, tracked)
}

/// The `purlis/<sha>` branch an earlier push is STILL waiting on a pull request for — or the
/// `charter/<sha>` one a push made before the rename is waiting on, which carries on until its
/// request merges (V93j).
///
/// Advancing it is a fast-forward — each new HEAD is a descendant of the one before — so one
/// open pull request accumulates the commits instead of leaving one abandoned remote branch
/// per save. Safe because it is only ever a FIRST attempt: the caller pushes without
/// `--force`, so git itself refuses anything that is not a fast-forward.
fn open_pull_request_branch(root: &Path) -> Option<String> {
    let rec = unlanded(root)?;
    if rec.get("outcome").and_then(serde_json::Value::as_str) != Some(Outcome::Branched.word()) {
        return None;
    }
    // Held to the name `land_via_branch` mints, for the reason `is_object_name` gives: this
    // value becomes the remote ref a push writes, and a record naming `main` would make the
    // next save advance the branch the whole pull-request path exists to leave alone.
    rec.get("landed")
        .and_then(serde_json::Value::as_str)
        .filter(|name| {
            crate::names::BRANCH_PREFIX
                .strip(name)
                .is_some_and(is_object_name)
        })
        .map(str::to_owned)
}

// --------------------------------------------------------------------------------------- //
// pushing                                                                                   //
// --------------------------------------------------------------------------------------- //

/// Signatures a forge uses to say "this branch requires a pull request". Matched, never
/// interpolated, and each one observed rather than imagined (ADR 0009: charter may name a
/// cause it RECOGNISED, never one it inferred). An unmatched rejection falls through to the
/// generic "push failed" warning, which costs precision and can never mislead.
const PROTECTED_SIGNATURES: [&str; 8] = [
    "protected branch",
    "gh006",
    // A GitHub ruleset that protects the branch (GH013). Only its branch-protection rule lines,
    // never GH013 itself: GH013 also refuses a secret (push protection) or a file a ruleset
    // forbids, and neither is a protected branch.
    "changes must be made through a pull request",
    "cannot update this protected ref",
    "pre-receive hook declined",
    "required status check",
    "merge_request",
    "not allowed to push",
];

pub(crate) fn is_protected_rejection(stderr: &str) -> bool {
    let blob = stderr.to_lowercase();
    PROTECTED_SIGNATURES.iter().any(|s| blob.contains(s))
}

/// The last four lines of what git said, which is what charter repeats.
pub(crate) fn tail(run: &git::Run) -> String {
    // stderr when there IS any, exactly as Python's `p.stderr or p.stdout or ""` chooses.
    let text = if run.err.is_empty() {
        &run.out
    } else {
        &run.err
    };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(4)..].join("\n")
}

/// Push the commit that is already on HEAD to a remote branch, and hand back a URL.
///
/// The sanctioned path for a control plane whose own repo requires pull requests. **The plane
/// root's HEAD never moves**: `git push HEAD:refs/heads/<new>` needs no checkout, no branch
/// creation and no worktree — only a different REMOTE ref for a commit that already exists.
fn land_via_branch(
    root: &Path,
    https: &str,
    helper: Option<&str>,
    default_branch: &str,
    say: Sink,
) -> PushResult {
    let sha = git::run(root, &["rev-parse", "--short", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let fresh = format!(
        "{}{}",
        crate::names::BRANCH_PREFIX.write,
        if sha.is_empty() { "change" } else { &sha }
    );
    let kind = forge::request_words_of(root, root);
    let noun = kind.request_noun();
    let reuse = open_pull_request_branch(root).filter(|r| *r != fresh);
    // A reuse that did not fast-forward is not a failure to report: the fresh name has not
    // been tried yet, and naming a branch charter chose on the operator's behalf as the thing
    // that went wrong would send the reader after a problem they do not have.
    let mut attempt: Option<(String, git::Run)> = None;
    for candidate in reuse.into_iter().chain(std::iter::once(fresh.clone())) {
        let pushed = git::run_network(
            root,
            helper,
            &["push", https, &format!("HEAD:refs/heads/{candidate}")],
        );
        if let Ok(run) = pushed {
            let landed = run.ok();
            attempt = Some((candidate, run));
            if landed {
                break;
            }
        }
    }
    if !attempt.as_ref().is_some_and(|(_, run)| run.ok()) {
        let said = attempt
            .as_ref()
            .map(|(_, run)| tail(run))
            .unwrap_or_default();
        say(Say::Fail(format!(
            "'{default_branch}' requires a {noun}, and pushing the branch '{fresh}' also failed:"
        )));
        for line in said.lines() {
            say(Say::Fail(format!("  {line}")));
        }
        return PushResult {
            detail: said,
            ..PushResult::of(Outcome::Stranded, default_branch)
        };
    }
    let branch = attempt.map(|(name, _)| name).unwrap_or(fresh);
    let url = compare_url(https, &branch, root);
    say(Say::Done(format!(
        "'{default_branch}' requires a {noun} — pushed {branch} instead."
    )));
    if let Some(url) = &url {
        say(Say::Info(format!("  open it: {url}")));
    }
    say(Say::Info(format!(
        "  the commit is also on your local {default_branch}, one ahead of the remote. After \
         the {} merges: git -C {} pull --rebase",
        kind.request_short(),
        root.display()
    )));
    PushResult {
        landed: Some(branch),
        url,
        ..PushResult::of(Outcome::Branched, default_branch)
    }
}

/// The deadline for every git call `save` makes that writes a commit — the commit, its unsigned
/// retry, the rebase a moved remote sends it through — and for the rebase's abort.
///
/// Not [`git::READ`]: a rebase checks out a tree, and a killed one leaves a rebase in progress
/// for the abort to clear. But not untimed either, which each of them was until
/// charter-app#242: a git that waits — on a signer, on a lock, on anything — held
/// `charter save`, and the hook that ran it, for ever. A plane's save commits a handful of
/// files and replays the few commits the remote does not have yet, so two minutes is ample
/// and still ends.
pub(crate) const WRITE: Duration = Duration::from_secs(120);

/// How the rebase onto a remote that moved ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Rebased {
    /// The plane's commits were replayed onto the remote's.
    Replayed,
    /// The trees conflict: git stopped with files it calls unmerged.
    /// Carries those files, read before the rebase is undone.
    Conflict(Vec<String>),
    /// git did not replay the commits for a reason that is no conflict: it could not write a
    /// file, the disk is full, it refused, or it died. Carries git's own words. A rebase may
    /// or may not have started ([`rebase_in_progress`] says), so whoever undoes it asks first.
    Failed(String),
    /// Asked to sign (`--sign`), git could not write a replayed commit: the signer refused.
    /// Carries what the signer said (charter-app#267).
    Unsigned(String),
    /// Still running at its deadline, and killed.
    OutOfTime,
    /// Never started: this process runs under a chat's sandbox, which denies it writing these
    /// files the remote's commits change (qw346).
    Held(Vec<String>),
}

/// Rebase the plane root onto `FETCH_HEAD`, giving up at `deadline`.
///
/// **Signed exactly when the commit was** (charter-app#242). A rebase REPLAYS commits and a
/// replay is a commit, so it reads `commit.gpgsign` as the commit did. Unless the operator
/// asked for `--sign`, `-c commit.gpgsign=false` keeps it from the signer the commit was kept
/// from — the commit `save` made unsigned came back out of the rebase asking for a 1Password
/// prompt that blocks, or a signer that fails and turns a remote that merely moved into a
/// reported conflict. With `--sign` the `-c` is left off, as it is for the commit, so the
/// replay is signed the way the commit was; putting it on would hand the remote an unsigned
/// copy of a commit the operator asked to sign.
///
/// **A signer that refuses is not a conflict** (charter-app#267). Under `--sign` a replay that
/// cannot be signed stops the rebase exactly where a conflict would, with no conflict in it:
/// git says `failed to write commit object` (in the C locale the runner pins) after the
/// signer's own words, and a conflict never says that. What the signer said is kept for the
/// operator; which signer, and why, only it knows. A commit object git failed to write for
/// another reason — a full disk — would be put down to the signer too, but git's own words
/// follow the claim, so the operator reads the real cause either way.
///
/// A run with no exit code is out of time only once the deadline has passed: git killed by a
/// signal of somebody else's has no code either, and that is not charter stopping it.
///
/// **Only a conflict is called a conflict** (qw348). Every failure used to be read as one, so
/// a file git could not write, a full disk or a git that died sent the reader to settle a
/// conflict that did not exist. A conflict is a rebase that stopped with unmerged files; any
/// other failure is [`Rebased::Failed`], in git's words.
fn rebase_onto_fetched(root: &Path, sign: bool, deadline: Duration) -> Rebased {
    rebase_onto(root, "FETCH_HEAD", sign, deadline)
}

/// [`rebase_onto_fetched`], onto `onto`: `FETCH_HEAD` for a save's retry, the remote-tracking
/// branch for the window's Update ([`update::update`]).
fn rebase_onto(root: &Path, onto: &str, sign: bool, deadline: Duration) -> Rebased {
    let stops = sandbox_would_stop(root, onto);
    if !stops.is_empty() {
        return Rebased::Held(stops);
    }
    let started = std::time::Instant::now();
    let mut rebase: Vec<&str> = vec!["-c", gpgsign(sign)];
    rebase.extend(["rebase", onto]);
    match git::run(root, &rebase, deadline) {
        Ok(run) if run.ok() => Rebased::Replayed,
        Ok(run) if run.code.is_none() && started.elapsed() >= deadline => Rebased::OutOfTime,
        Ok(run) if sign && run.err.contains(UNWRITTEN) => Rebased::Unsigned(signer_said(&run.err)),
        Ok(run) => match gitstate::unmerged(root) {
            unmerged if unmerged.is_empty() => Rebased::Failed(tail(&run)),
            unmerged => Rebased::Conflict(unmerged),
        },
        Err(unavailable) => Rebased::Failed(unavailable.to_string()),
    }
}

/// Whether git has a rebase stopped part-way in the repository at `root`: the one thing
/// `git rebase --abort` undoes. Asked before every abort: where no rebase started, the abort
/// itself fails, and its failure used to be all that was done about a tree the failed checkout
/// had left half-written (qw348).
fn rebase_in_progress(root: &Path) -> bool {
    let git_dir = git_dir_of(root);
    git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir()
}

/// The `-c` that decides signing for one git command, beating the operator's own
/// `commit.gpgsign` either way: a save that is not asked to sign never runs a signer, and one
/// that is — `--sign`, or `[plane] sign = true` (ADR 0051) — is signed whatever the machine's
/// default says.
pub(crate) fn gpgsign(sign: bool) -> &'static str {
    if sign {
        "commit.gpgsign=true"
    } else {
        "commit.gpgsign=false"
    }
}

/// What git's sequencer says when it cannot write a replayed commit — under `--sign`, because
/// the signer refused. `sequencer.c`'s own message, read in the C locale.
const UNWRITTEN: &str = "failed to write commit object";

/// The signer's words out of a rebase that stopped on it: everything git printed before
/// [`UNWRITTEN`], without its progress, its hints or its `error: ` prefixes.
///
/// git prints `Rebasing (1/1)` with a carriage return and no newline, so the line the signer's
/// error lands on starts with it; a line is read from its last `\r`.
pub(crate) fn signer_said(err: &str) -> String {
    let before = err.split(UNWRITTEN).next().unwrap_or_default();
    let said: Vec<&str> = before
        .lines()
        .map(|line| line.rsplit('\r').next().unwrap_or_default().trim())
        .map(|line| line.strip_prefix("error: ").unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with("hint:") && *line != "error:")
        .collect();
    if said.is_empty() {
        "the signer gave no reason".to_string()
    } else {
        said.join(" ")
    }
}

/// A rebase that stopped for a reason other than a conflict, undone: aborted, the operator
/// told `detail` and what to do `next`, and recorded as `failed` with that detail — the
/// commit is on this laptop only.
fn rebase_undone(
    root: &Path,
    branch: &str,
    head: &str,
    detail: String,
    next: &str,
    say: Sink,
) -> PushResult {
    // The save's own commit is HEAD, and the tree was clean when it was made: put back to it.
    let detail = match update::put_back(root, head, "FETCH_HEAD") {
        Ok(()) => detail,
        Err(left) => format!("{detail}; {}", update::said_not_put_back(&left)),
    };
    say(Say::Warn(format!("Committed locally, but {detail}.")));
    say(Say::Info(format!("  {next}")));
    record_push(
        root,
        PushResult {
            detail,
            ..PushResult::of(Outcome::Failed, branch)
        },
        head,
    )
}

/// The journal's and the push record's words for a save's push the secret scan refused —
/// matched whole, never searched for.
pub const PUSH_SECRET_REFUSED: &str = "a secret-shaped value in a commit the push would send";

/// **What a save's own push would send, scanned before it is sent** (SQ-7), or `None` when the
/// push may go on. Pushes purlis makes run git with no hooks, so the `pre-push` scan a chat's
/// push gets never runs for them: a commit made by hand in the plane, then pushed by a save,
/// was published unscanned.
///
/// The scan is [`crate::diffscan::pushed`]'s, over HEAD as `branch` on `origin` (at `url`),
/// past what any of `origin`'s tracking refs say it has. **It refuses on a credential only**: the
/// save's own scan lets personal data through, as a plane's memory names people, so a push
/// does not refuse what the save just committed. A scan git could not answer refuses too (fail
/// closed), and says why. Refused, nothing is pushed, the findings are said masked, and the
/// outcome is recorded as `blocked`, with [`PUSH_SECRET_REFUSED`] for a finding.
fn refused_push(root: &Path, branch: &str, url: &str, head: &str, say: Sink) -> Option<PushResult> {
    // What origin has is what its tracking refs say, all of them (`--not --remotes=origin`): so
    // the remote's side of the one ref is left as nothing.
    let theirs = "0".repeat(head.len().max(40));
    let updates = format!("HEAD {head} refs/heads/{branch} {theirs}\n");
    let refused: Vec<crate::diffscan::Finding> =
        match crate::diffscan::pushed(root, "origin", url, &updates) {
            Ok(scan) => scan
                .refused
                .into_iter()
                .filter(|finding| !secretshape::is_personal(finding.rule))
                .collect(),
            Err(why) => {
                let detail = format!("purlis could not scan what the push would send: {why}");
                say(Say::Fail(format!("Not pushed — {detail}")));
                return Some(record_push(
                    root,
                    PushResult {
                        detail,
                        ..PushResult::of(Outcome::Blocked, branch)
                    },
                    head,
                ));
            }
        };
    if refused.is_empty() {
        return None;
    }
    say(Say::Fail(
        "Not pushed — a commit the push would send holds what looks like a secret:".into(),
    ));
    for finding in refused.iter().take(20) {
        say(Say::Fail(format!(
            "  {}",
            shown::readable(&crate::diffscan::one_line(finding), shown::DISPLAY_LIMIT)
        )));
    }
    if refused.len() > 20 {
        say(Say::Fail(format!("  … and {} more", refused.len() - 20)));
    }
    say(Say::Info(
        "It is committed on this machine only. Take it out of that commit's history (amend it, \
         or rebase and edit the commit that added it), revoke it if it is a live credential, \
         then `purlis save`. Secrets belong in the vault (`purlis persona secret set`)."
            .into(),
    ));
    Some(record_push(
        root,
        PushResult {
            detail: PUSH_SECRET_REFUSED.into(),
            ..PushResult::of(Outcome::Blocked, branch)
        },
        head,
    ))
}

/// [`prsave::push`], after [`refused_push`]'s scan of what it would send, where the plane has an
/// origin to push to at all.
fn push_for_a_request(
    root: &Path,
    plane: &crate::planesave::Plane,
    sign: bool,
    caller: &forge::Caller,
    say: Sink,
) -> PushResult {
    if let Some(url) = origin_https(root) {
        let head = git::run(root, &["rev-parse", "HEAD"], git::READ)
            .map(|r| r.line().trim().to_string())
            .unwrap_or_default();
        if let Some(refused) = refused_push(root, "", &url, &head, say) {
            return said_if_unrecorded(refused, say);
        }
    }
    prsave::push(root, plane, sign, caller, say)
}

/// Push the plane root's HEAD to its own branch on origin — **the one pusher**.
///
/// **The branch is never predicted.** Nothing here asks the forge whether the branch is
/// protected before committing: charter cannot know that without a network call it has no
/// business making from a hook, and guessing it from the branch name is the unearned diagnosis
/// ADR 0009 forbids. The rejection IS the evidence, and it arrives only after the commit
/// exists — so the commit is made, and the OUTCOME is what gets reported honestly.
///
/// `sign` is the `--sign` the commit was made under: a remote that moved sends HEAD through a
/// rebase, and the commit it replays is signed exactly when the commit was.
///
/// `target` is the branch to advance: `[plane] branch`, or `None` for the one HEAD is on.
pub fn push_head(root: &Path, target: Option<&str>, sign: bool, say: Sink) -> PushResult {
    let res = push_head_within(root, target, sign, WRITE, say);
    said_if_unrecorded(res, say)
}

/// [`push_head`], with the rebase given `deadline` rather than [`WRITE`] — so a test can drive
/// a rebase that runs out of time without waiting out the real one.
fn push_head_within(
    root: &Path,
    target: Option<&str>,
    sign: bool,
    deadline: Duration,
    say: Sink,
) -> PushResult {
    let branch = match target {
        Some(target) => target.to_string(),
        None => git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
            .map(|r| r.line().trim().to_string())
            .unwrap_or_default(),
    };
    let mut head = git::run(root, &["rev-parse", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let Some(https) = origin_https(root) else {
        say(Say::Warn(
            "origin isn't on a forge purlis knows (gitlab.com/github.com/…) — committed \
             locally; push manually."
                .into(),
        ));
        // Returned UNRECORDED, deliberately. `unreachable` is the one outcome `doctor` never
        // reports — a plane with no origin charter knows has a CONFIGURATION to fix, not a
        // commit to rescue — so writing it could only ever DESTROY a real notice, and it did.
        return PushResult::of(Outcome::Unreachable, &branch);
    };
    if let Some(refused) = refused_push(root, &branch, &https, &head, say) {
        return refused;
    }
    // The forge of the plane's OWN origin, and its CLI's helper by absolute path. One
    // credential per forge: a GitHub-hosted plane authenticates with `gh`, never with
    // whichever helper happens to be first in the operator's config.
    // `origin_https` answered, so the host IS one this plane manages and `forge_for` resolves
    // it; the fallback is here so a future divergence between the two cannot panic a push.
    let forge = crate::gitpolicy::forge_for(root, root)
        .unwrap_or_else(|| Forge::default_of(forge::DEFAULT_KIND));
    // The window's own sign-in goes only to the remote recorded for this plane (#752).
    let helper = match forge::gitcredential::helper_for_push(&forge, root, &https) {
        Ok(helper) => helper,
        Err(why) => {
            say(Say::Warn(format!("Committed, but not pushed: {why}.")));
            return PushResult {
                detail: why.to_string(),
                ..PushResult::of(Outcome::Blocked, &branch)
            };
        }
    };
    let cli = forge.kind.cli();
    let push = |root: &Path| {
        git::run_network(
            root,
            Some(helper.as_str()),
            &["push", &https, &format!("HEAD:{branch}")],
        )
    };

    let mut run = match push(root) {
        Ok(run) => run,
        Err(unavailable) => {
            say(Say::Warn(format!("Committed, but the {cli} push failed:")));
            say(Say::Warn(format!("  {unavailable}")));
            say(Say::Info(format!("Check `{cli} auth status`.")));
            return record_push(
                root,
                PushResult {
                    detail: unavailable.to_string(),
                    ..PushResult::of(Outcome::Failed, &branch)
                },
                &head,
            );
        }
    };
    let said = |run: &git::Run| {
        if run.err.is_empty() {
            run.out.clone()
        } else {
            run.err.clone()
        }
    };
    if !run.ok() && is_protected_rejection(&said(&run)) {
        // Asked BEFORE the non-fast-forward retry, and the ordering is load-bearing: git
        // prints its own `! [remote rejected]` line above a server-side hook decline, so the
        // word "rejected" in the retry test below matches a protected branch too. Measured
        // against a real bare remote with a pre-receive hook refusing `refs/heads/main`:
        // taking the retry first fetched an unreachable origin, which REMOVES FETCH_HEAD, so
        // the rebase failed and the outcome was recorded as `conflict` — the pull-request path
        // never reached and charter telling the operator to resolve a conflict that did not
        // exist.
        return record_push(
            root,
            land_via_branch(root, &https, Some(helper.as_str()), &branch, say),
            &head,
        );
    }
    let moved = ["fetch first", "non-fast-forward", "rejected"];
    if !run.ok() && moved.iter().any(|s| run.err.contains(*s)) {
        say(Say::Info(
            "remote moved — fetching + rebasing, then retrying …".into(),
        ));
        let fetched = git::run_network(root, Some(helper.as_str()), &["fetch", &https, &branch]);
        if !fetched.is_ok_and(|r| r.ok()) {
            // A failed fetch leaves NO FETCH_HEAD to rebase onto, so rebasing anyway fails for
            // a reason that has nothing to do with a conflict. Calling that `conflict` would
            // send the reader to resolve one that does not exist while the real answer — the
            // push failure below, in git's own words — is discarded.
            say(Say::Warn(
                "Could not reach origin to fetch, so the retry was skipped.".into(),
            ));
        } else {
            match rebase_onto_fetched(root, sign, deadline) {
                Rebased::Replayed => {}
                Rebased::Conflict(conflicts) => {
                    let _ = git::run(root, &["rebase", "--abort"], WRITE);
                    say(Say::Warn(
                        "Committed locally, but rebase hit a conflict — resolve manually, then \
                         `purlis save`."
                            .into(),
                    ));
                    let detail = if conflicts.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "the remote changed the same lines in: {}",
                            conflicts.join(", ")
                        )
                    };
                    return record_push(
                        root,
                        PushResult {
                            detail,
                            conflicts,
                            ..PushResult::of(Outcome::Conflict, &branch)
                        },
                        &head,
                    );
                }
                Rebased::Unsigned(signer) => {
                    // Stopped, not retried unsigned as the commit is: the commit stays on this
                    // laptop, where re-signing it is the operator's to do, while a push is
                    // public and final. The remote never gets an unsigned copy of a commit the
                    // operator asked to sign.
                    return rebase_undone(
                        root,
                        &branch,
                        &head,
                        format!("the rebase could not sign the replayed commit: {signer}"),
                        "Nothing was pushed, so nothing unsigned reached the remote. Fix the \
                         signer, then rebase onto the remote and push by hand.",
                        say,
                    );
                }
                Rebased::Failed(said) => {
                    // Not a conflict, and not called one (qw348): git's own words say what
                    // stopped it. Whatever the failed checkout wrote is put back.
                    return rebase_undone(
                        root,
                        &branch,
                        &head,
                        format!("the rebase onto the remote did not go through: {said}"),
                        "Nothing was pushed. Settle what git names, then `purlis save`.",
                        say,
                    );
                }
                Rebased::Held(files) => {
                    // Nothing was started, so there is nothing to undo. Recorded as failed,
                    // not blocked: the window's own save runs under no sandbox and lands it.
                    let detail = said_sandbox_stops(&files);
                    say(Say::Warn(format!("Committed locally, but {detail}.")));
                    say(Say::Info(
                        "  Nothing was rebased or pushed. The window's save brings the remote's \
                         commits in and pushes this one, or the person runs `purlis save` in \
                         their own terminal."
                            .into(),
                    ));
                    return record_push(
                        root,
                        PushResult {
                            detail,
                            ..PushResult::of(Outcome::Failed, &branch)
                        },
                        &head,
                    );
                }
                Rebased::OutOfTime => {
                    // Not a conflict, and not called one: nothing says the trees disagree,
                    // only that git did not finish. The push that started this is what failed
                    // to land.
                    return rebase_undone(
                        root,
                        &branch,
                        &head,
                        format!(
                            "the rebase onto the remote did not finish within {} seconds, so \
                             purlis stopped it",
                            deadline.as_secs()
                        ),
                        "Rebase by hand, then `purlis save`.",
                        say,
                    );
                }
            }
            // The rebase rewrote it, so the commit the record names is a different one now.
            head = git::run(root, &["rev-parse", "HEAD"], git::READ)
                .map(|r| r.line().trim().to_string())
                .unwrap_or_default();
            match push(root) {
                Ok(again) => run = again,
                Err(_) => {
                    return record_push(root, PushResult::of(Outcome::Failed, &branch), &head);
                }
            }
        }
    }
    if run.ok() {
        // Keep the tracking ref in step, as Python does, so the next status is not one behind.
        let _ = git::run(
            root,
            &[
                "update-ref",
                &format!("refs/remotes/origin/{branch}"),
                "HEAD",
            ],
            git::READ,
        );
        say(Say::Done(format!(
            "Pushed {branch} via {cli} (HTTPS token — no SSH, no 1Password)."
        )));
        return record_push(root, PushResult::of(Outcome::Pushed, &branch), &head);
    }
    if is_protected_rejection(&said(&run)) {
        // Not redundant with the identical check above the retry: the FIRST push can fail
        // plain non-fast-forward, and the SECOND — after a rebase that succeeded — is the one
        // the protected branch refuses. Same policy, reached from the other side.
        return record_push(
            root,
            land_via_branch(root, &https, Some(helper.as_str()), &branch, say),
            &head,
        );
    }
    let detail = tail(&run);
    say(Say::Warn(format!("Committed, but the {cli} push failed:")));
    for line in detail.lines() {
        say(Say::Warn(format!("  {line}")));
    }
    say(Say::Info(format!("Check `{cli} auth status`.")));
    record_push(
        root,
        PushResult {
            detail,
            ..PushResult::of(Outcome::Failed, &branch)
        },
        &head,
    )
}

// --------------------------------------------------------------------------------------- //
// staging                                                                                   //
// --------------------------------------------------------------------------------------- //

/// Pathspecs that name the whole repository rather than a path inside it. `git add .` run with
/// `-C <root>` is `git add -A` wearing a different spelling, and `:/` says so outright.
const WHOLE_TREE_PATHSPECS: [&str; 4] = [".", "./", ":/", ":/:"];

/// Flags that, with NO pathspec, make git operate on the whole tree instead of on nothing.
/// `-u` is here with `-A`: it stages only tracked modifications, which is precisely the
/// operator's mid-edit `charter.toml`.
const WHOLE_TREE_FLAGS: [&str; 5] = ["-A", "--all", "--no-ignore-removal", "-u", "--update"];

/// **What a save never stages** ([`save_stages`]): every level's local skills (ADR 0099 §1,
/// #2767; a persona's, #3124), spelled by the one level rule ([`purlis_skills::local_globs`]),
/// and the lockfile of the skills installed there (ADR 0100 §2, #2773), whatever the plane's
/// `.gitignore` says, since `skills.local/` is this machine's own and a plane whose `.gitignore`
/// lost its line would otherwise commit it to the team; and a Try it's throwaway pieces (#2772),
/// which last as long as their chats.
pub fn never_staged() -> Vec<String> {
    purlis_skills::local_globs()
        .iter()
        .map(|dir| format!(":(exclude,glob){dir}/**"))
        .chain([
            ":(exclude,literal)skills.local.lock".to_owned(),
            ":(exclude,glob)workspaces/.throwaway/**".to_owned(),
        ])
        .collect()
}

/// **What a plane save stages**: the whole tree, but what is [`never_staged`].
pub fn save_stages() -> Vec<String> {
    ["add", "-A", "--", "."]
        .into_iter()
        .map(str::to_owned)
        .chain(never_staged())
        .collect()
}

/// Would `add_cmd` stage EVERYTHING under the root it is run in?
///
/// The property that makes a call dangerous is not which command issued it — it is that it
/// sweeps up files the caller never named and, from another working tree, cannot see. So the
/// worktree refusal is keyed on this rather than on `charter save`, and the next caller to copy
/// that shape inherits the guard.
///
/// **With no pathspec, the flags decide, and a bare `--` bounds nothing.** Measured against git
/// on a repo with one edited tracked file and one untracked file: `git add` and `git add --`
/// stage nothing, `git add -A` and `git add -A --` stage both, `git add -u` stages the tracked
/// edit. The first draft treated `["add", "-A", "--"]` as scoped, and it stages the whole tree.
pub fn stages_the_whole_tree(add_cmd: &[&str]) -> bool {
    let args = &add_cmd[1.min(add_cmd.len())..];
    let (flags, paths): (Vec<&str>, Vec<&str>) = match args.iter().position(|a| *a == "--") {
        // Everything past the separator is a pathspec, even one that starts with a dash: a
        // file really can be called `-A`, and past `--` git stages that file rather than
        // reading the flag.
        Some(i) => (args[..i].to_vec(), args[i + 1..].to_vec()),
        None => (
            args.to_vec(),
            args.iter()
                .copied()
                .filter(|a| !a.starts_with('-'))
                .collect(),
        ),
    };
    if !paths.is_empty() {
        // `any`, not `all`: measured, `git add -- . u.txt` stages the whole tree. One
        // whole-tree spelling anywhere in the list widens the call, and the named paths beside
        // it change nothing.
        return paths.iter().any(|p| WHOLE_TREE_PATHSPECS.contains(p));
    }
    flags.iter().any(|f| WHOLE_TREE_FLAGS.contains(f))
}

/// The linked worktree of `plane`'s repository that `start` stands in, else `None`.
///
/// A plane and a working tree are not the same object. The PLANE is identity and
/// machine-local state; the TREE is committed content on a branch. `save` commits, so an
/// unbounded stage typed in a worktree would commit the plane's clone — the operator's
/// half-finished files under the agent's message — and none of the agent's own work.
pub fn tree_of(plane: &Path, start: &Path) -> Option<PathBuf> {
    let here = start.canonicalize().ok()?;
    let target = plane.canonicalize().ok()?;
    here.ancestors()
        .find(|d| crate::plane::main_worktree_of(d).as_deref() == Some(target.as_path()))
        .map(Path::to_path_buf)
}

/// The plane nested inside `plane`'s `workspaces/` that `start` stands in, else `None`.
///
/// [`tree_of`]'s sibling and deliberately a SECOND route rather than a widening of it: a
/// linked worktree is marked by a `.git` file, and a nested plane is a different repository
/// with a real `.git` directory that happens to carry a tracked `charter.toml` — which every
/// clone of a plane does. `charter clone` puts clones at `workspaces/<ws>/<repo>`, and charter
/// dogfoods on a clone of itself, so this is an everyday layout rather than an exotic one.
///
/// Root-relative, exactly like [`tree_of`], and that is what makes `$CHARTER_ROOT` work: with
/// the plane being committed set to the one the caller stands in, this answers `None` and the
/// operator gets what they explicitly asked for.
pub fn nested_plane_in(plane: &Path, start: &Path) -> Option<PathBuf> {
    let here = start.canonicalize().ok()?;
    let target = plane.canonicalize().ok()?;
    let marked = here.ancestors().find(|d| crate::names::has_manifest(d))?;
    let inner = crate::plane::plane_of(marked);
    // The chain is WALKED rather than shortcut through `outermost`: in a plane inside a plane
    // inside a plane, under `$CHARTER_ROOT=<the middle one>`, the outermost is not the tree
    // being committed, yet standing in the leaf still commits a tree the caller is not in.
    let mut cur = inner.clone();
    loop {
        let outer = crate::plane::enclosing(&cur)?;
        if outer == target {
            return Some(inner);
        }
        cur = outer;
    }
}

// --------------------------------------------------------------------------------------- //
// the command                                                                               //
// --------------------------------------------------------------------------------------- //

/// What `charter save` was asked for.
pub struct Request<'a> {
    pub root: &'a Path,
    /// The commit message, or `None` for `charter save: N file(s)`.
    pub message: Option<&'a str>,
    /// Sign the commit. Off by default so a signer prompt can never hang an agent.
    pub sign: bool,
    /// Commit only; do not push.
    pub no_push: bool,
    /// Where the operator is standing — what the two refusals below are about.
    pub cwd: &'a Path,
    /// The agent run this save is for, when a chat's `charter save` asked for it: its trailers
    /// go on the commit (GL-8, V67). `None` for every save the operator or the app made.
    pub provenance: Option<&'a crate::provenance::Provenance>,
}

/// `charter save`: [`save_as`] started from the command line.
pub fn save(request: &Request, say: Sink) -> u8 {
    save_as(request, Trigger::Cli, say)
}

/// The planes a save — or a fetch — is running in, in this process, so a second one waits its
/// turn in words rather than racing the first for git's index lock (charter-app#294, #296).
static RUNNING: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// The file in a repository's git directory whose lock a save or a fetch of it holds, so that
/// two processes never run one at once (#2002): the window, `purlis save` in a terminal and
/// the session host's auto-save are three.
pub const SAVE_LOCK: &str = "purlis-save.lock";

/// A save or a fetch running in one plane, for as long as it is held: in this process, and,
/// where the plane is a repository, against every other process too ([`SAVE_LOCK`]).
#[derive(Debug)]
pub struct Claim {
    root: PathBuf,
    /// The repository's lock this claim shares in this process, while it is held.
    across: Option<PathBuf>,
}

/// The [`SAVE_LOCK`]s this process holds, by the git directory, with how many claims share
/// each (#2002). The lock is between processes: within one, a claim's rule is [`RUNNING`]'s, as
/// it was before there was a lock, so two claims of one repository under two spellings (a
/// nested one, reaching it through a link) share the lock rather than refuse each other.
static ACROSS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, (usize, crate::filelock::Held)>>,
> = std::sync::LazyLock::new(Default::default);

impl Claim {
    /// The plane at `root`, unless something already holds it, in this process or another.
    pub fn of(root: &Path) -> Option<Self> {
        let running = || {
            RUNNING
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        if !running().insert(root.to_path_buf()) {
            return None;
        }
        match across_processes(root) {
            Ok(across) => Some(Self {
                root: root.to_path_buf(),
                across,
            }),
            Err(HeldElsewhere) => {
                running().remove(root);
                None
            }
        }
    }

    /// The plane at `root`, waiting up to `bound` for whatever holds it to let go.
    pub fn within(root: &Path, bound: Duration) -> Option<Self> {
        let until = std::time::Instant::now() + bound;
        loop {
            if let Some(claim) = Self::of(root) {
                return Some(claim);
            }
            if std::time::Instant::now() >= until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Another process holds the plane's [`SAVE_LOCK`].
struct HeldElsewhere;

/// A share of the lock on the [`SAVE_LOCK`] of the repository at `root`, by its git directory,
/// or [`HeldElsewhere`] when another process holds it. A folder that is no repository has no
/// git directory to lock and nothing a save could race on, and a lock file that cannot be made
/// or locked for another reason leaves the claim to this process alone, as it was before there
/// was a lock: neither is a refusal.
fn across_processes(root: &Path) -> Result<Option<PathBuf>, HeldElsewhere> {
    let Some(git_dir) = root
        .to_str()
        .and_then(crate::gitconfig::git_dir_at)
        .map(PathBuf::from)
    else {
        return Ok(None);
    };
    let git_dir = git_dir.canonicalize().unwrap_or(git_dir);
    let mut across = ACROSS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((shares, _)) = across.get_mut(&git_dir) {
        *shares += 1;
        return Ok(Some(git_dir));
    }
    let Ok(lock) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(git_dir.join(SAVE_LOCK))
    else {
        return Ok(None);
    };
    match lock.try_lock() {
        Ok(()) => {
            across.insert(git_dir.clone(), (1, crate::filelock::Held::locked(lock)));
            Ok(Some(git_dir))
        }
        Err(std::fs::TryLockError::WouldBlock) => Err(HeldElsewhere),
        Err(std::fs::TryLockError::Error(_)) => Ok(None),
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        // Other processes first, once no claim here shares the lock: the lock is let go of
        // explicitly (`filelock::Held`), so a copy of its descriptor in a child this process is
        // forking does not keep it held.
        if let Some(git_dir) = self.across.take() {
            let mut across = ACROSS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let last = across.get_mut(&git_dir).is_some_and(|(shares, _)| {
                *shares -= 1;
                *shares == 0
            });
            if last {
                across.remove(&git_dir);
            }
        }
        RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.root);
        // Every save and fetch of a plane or a clone holds its claim, so letting go of one is
        // when what it committed, pushed or fetched is there to be read (FD-11).
        touch(&self.root);
    }
}

/// The journal's words for a save the secret scan refused — matched whole, never searched for.
pub const SECRET_REFUSED: &str = "a secret-shaped value in a staged file";

/// The most bytes of a file outside memory and refs the secret guard reads. Larger ones are
/// committed unscanned and named in the save's output.
pub const SCAN_LIMIT: usize = 10 << 20;

/// How far into a file git looks for a NUL before it calls the file binary, and so does the
/// secret guard.
const BINARY_PROBE: usize = 8000;

/// A memory or ref file: scanned whole whatever its size or bytes, and refused when it is a
/// link, as it was before every staged file was scanned.
fn memory_or_ref(path: &str) -> bool {
    path.contains("/memory/") || path.contains("/refs/")
}

/// Why the secret guard commits a file without reading it, or `None` when it reads it: a file
/// outside memory and refs that git calls binary, or that is over [`SCAN_LIMIT`]. The one rule
/// the save and the standing both ask.
fn unscanned(path: &str, bytes: &[u8]) -> Option<&'static str> {
    if memory_or_ref(path) {
        None
    } else if bytes.len() > SCAN_LIMIT {
        Some("over 10 MiB")
    } else if bytes.iter().take(BINARY_PROBE).any(|b| *b == 0) {
        Some("binary")
    } else {
        None
    }
}

/// Whether the working-tree file at `path` holds what the next save's guard would refuse.
/// Outside memory and refs only a regular file within [`SCAN_LIMIT`] is read, so a link to a
/// device or a huge export never stalls the standing.
fn secret_on_disk(root: &Path, path: &str) -> bool {
    let file = root.join(path);
    if !memory_or_ref(path)
        && !std::fs::symlink_metadata(&file).is_ok_and(|meta| {
            meta.is_file() && usize::try_from(meta.len()).is_ok_and(|len| len <= SCAN_LIMIT)
        })
    {
        return false;
    }
    remembered(&file, || {
        std::fs::read(&file).is_ok_and(|bytes| {
            unscanned(path, &bytes).is_none()
                && secret_in(path, &String::from_utf8_lossy(&bytes)).is_some()
        })
    })
}

/// What changes whenever a file's content does: its length, its modification time, and on
/// Unix its inode and change time — the change time moves on every write and no caller can
/// set it back, so a rewrite of the same length within one tick is still a change.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl Stamp {
    fn of(file: &Path) -> Option<Self> {
        let meta = std::fs::metadata(file).ok()?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            #[cfg(unix)]
            inode: meta.ino(),
            #[cfg(unix)]
            changed: (meta.ctime(), meta.ctime_nsec()),
        })
    }
}

/// The most files whose standing answer is remembered; past it the memory starts again.
const REMEMBERED: usize = 4096;

/// How long ago a file must have been modified for its answer to be remembered. A file's
/// times move in ticks of the clock its filesystem keeps, and a second write within one
/// tick of the first can leave every time as it was — git's "racily clean" — so an answer
/// about a file that recent is never kept.
const SETTLED: Duration = Duration::from_secs(3);

/// `ask`'s answer for `file`, asked again only once the file has changed ([`Stamp`]) — the
/// standing is read often and a changed JSON or TOML file is parsed each time it is asked
/// (#1304). Only the standing remembers: a save asks every staged file every time. A file
/// whose metadata cannot be read, or that changed in the last [`SETTLED`], is asked every
/// time.
fn remembered(file: &Path, ask: impl FnOnce() -> bool) -> bool {
    use std::sync::{Mutex, OnceLock};
    static ANSWERS: OnceLock<Mutex<std::collections::HashMap<PathBuf, (Stamp, bool)>>> =
        OnceLock::new();
    let Some(before) = Stamp::of(file) else {
        return ask();
    };
    let answers = ANSWERS.get_or_init(Mutex::default);
    let known = answers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(file)
        .filter(|(stamp, _)| *stamp == before)
        .map(|(_, answer)| *answer);
    if let Some(answer) = known {
        return answer;
    }
    let answer = ask();
    // Remembered only for a file that has settled and did not change while it was read.
    let settled = before
        .modified
        .and_then(|at| SystemTime::now().duration_since(at).ok())
        .is_some_and(|age| age >= SETTLED);
    if settled && Stamp::of(file) == Some(before) {
        let mut answers = answers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if answers.len() >= REMEMBERED {
            answers.clear();
        }
        answers.insert(file.to_path_buf(), (before, answer));
    }
    answer
}

/// What the secret guard finds in the text of the file at `path`: its kind, and the line it
/// starts on when the text spells it. The one question the save and the standing both ask.
///
/// The text first, as it always was. Then what it spells through its escapes
/// ([`secretshape::decoded_kind`]): the parsed document for a file whose name says JSON or
/// TOML (#1295), and the text with its escapes decoded for every file, parsed or not (#1304) —
/// asked only when the text was clean, and with no line, since the decoded text's lines are
/// not the file's. The settings editors ask the same of what is typed into them.
fn secret_in(path: &str, text: &str) -> Option<(Option<usize>, &'static str)> {
    secretshape::found_as_read(secretshape::Structured::of(path), text)
}

/// How many `git show`s the secret guard runs at once. Each is a process of its own, and a
/// save that stages thousands of files spends its time starting them, not scanning.
const READERS: usize = 8;

/// What the index holds for each of `paths`, in their order: `None` for one git could not
/// show. `:<path>` is the index's own blob for that path, resolved from the top of the tree,
/// which is what `--name-only` printed and what `-C root` puts git in. Read-only, so the reads
/// run side by side.
fn staged_blobs(root: &Path, paths: &[&String]) -> Vec<Option<String>> {
    let show = |path: &String| {
        git::run(root, &["show", &format!(":{path}")], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.out)
    };
    let readers = std::thread::available_parallelism().map_or(1, |n| n.get().min(READERS));
    let chunk = paths.len().div_ceil(readers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                let reader = scope.spawn(move || part.iter().map(|p| show(p)).collect::<Vec<_>>());
                (part.len(), reader)
            })
            .collect();
        // A reader that panicked answers `None` for every path it held, so each of them is
        // refused as unread rather than dropped from the list and never scanned.
        handles
            .into_iter()
            .flat_map(|(len, reader)| reader.join().unwrap_or_else(|_| vec![None; len]))
            .collect()
    })
}

/// The sentence a save refused for [`Claim`] says.
pub const ALREADY_SAVING: &str = "A save of this plane is already running — wait for it to finish.";

/// What started a save — the journal's `trigger`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The window's save button.
    Manual,
    /// Auto-save, after the quiet period.
    Quiet,
    /// Auto-save, when a session ended.
    SessionEnd,
    /// Auto-save, as the app quit.
    Quit,
    /// A launch pushing what the last run left unpushed.
    Launch,
    /// `charter save`.
    Cli,
    /// A workspace going LIVE or LOCAL.
    Live,
    /// A workspace renamed (`charter workspace rename`), whose tracked files moved.
    Rename,
    /// A skill made, edited, renamed, moved, deleted, restored, granted or turned off (#2770):
    /// from the Skill studio in the window, or by `purlis skill`.
    Skill(crate::skills::verbs::By),
}

impl Trigger {
    /// Who the forge is asked as for a save this started (ADR 0070 §5): the person in the
    /// window for the save button and the window's LIVE switch, so their purlis sign-in opens
    /// the request (#1816); a `purlis` command for `purlis save` and a rename; and a trigger,
    /// which never gets a sign-in, for every save nobody pressed for.
    pub fn caller(self) -> forge::Caller {
        match self {
            Self::Manual | Self::Live | Self::Skill(crate::skills::verbs::By::Window) => {
                forge::Caller::window()
            }
            Self::Cli | Self::Rename | Self::Skill(crate::skills::verbs::By::Command) => {
                forge::Caller::command()
            }
            Self::Quiet | Self::SessionEnd | Self::Quit | Self::Launch => forge::Caller::trigger(),
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Quiet => "quiet",
            Self::SessionEnd => "session-end",
            Self::Quit => "quit",
            Self::Launch => "launch",
            Self::Cli => "cli",
            Self::Live => "live",
            Self::Rename => "rename",
            Self::Skill(_) => "skill",
        }
    }
}

/// Every save of the plane: the one save function (ADR 0051). Returns the exit status, and
/// leaves one line in the save journal ([`journal`]) saying how it ended.
///
/// How far it goes is `[plane] mode` ([`crate::planesave`]). A plane that names no mode is
/// saved as `charter save` always saved it — committed and pushed — because a bare command is
/// the operator asking for exactly that; only the app asks such a plane which mode it wants.
pub fn save_as(request: &Request, trigger: Trigger, say: Sink) -> u8 {
    let Some(claim) = Claim::of(request.root) else {
        say(Say::Fail(ALREADY_SAVING.into()));
        return 1;
    };
    save_claimed(request, trigger, &claim, say)
}

/// [`save_as`], for a caller that already holds the plane's [`Claim`].
pub fn save_claimed(request: &Request, trigger: Trigger, claim: &Claim, say: Sink) -> u8 {
    let stages = save_stages();
    let stages: Vec<&str> = stages.iter().map(String::as_str).collect();
    save_staging(request, trigger, claim, &stages, say)
}

/// What a save of exactly some paths says when something else is staged already.
pub const OTHERS_STAGED: &str = "Not saved: other changes are staged in this project, and this \
     save commits only its own files. Commit or unstage them, and the next save takes this one \
     too.";

/// **A save of exactly `paths`** (#2770), relative to the plane, `/` between their parts: one
/// commit that holds those paths and nothing else, through the one save ([`save_as`]), so the
/// plane's mode, its secret guard, its push and its journal are a skill change's too. The
/// paths-scoped staging of `doctor --fix rename-plane`'s commit, in the save itself.
///
/// A path is staged where git tracks something under it, or where it is on disk and git does
/// not ignore it: a removed folder's files leave the index, a new one's join it, and a local
/// file (a `skills.local/`, a workspace that is not LIVE) is never named, and each is named
/// literally. Refused, with nothing staged, for each reason [`paths_unsaved`] gives.
pub fn save_paths(request: &Request, trigger: Trigger, paths: &[String], say: Sink) -> u8 {
    let Some(claim) = Claim::of(request.root) else {
        say(Say::Fail(ALREADY_SAVING.into()));
        return 1;
    };
    let root = request.root;
    if let Some(why) = paths_unsaved(root, paths) {
        say(Say::Fail(why));
        return 1;
    }
    let named: Vec<String> = paths
        .iter()
        .filter(|path| !kept_out(path) && stageable(root, path))
        .cloned()
        .collect();
    if named.is_empty() {
        say(Say::Info(
            "Nothing to save: the change is in files that stay on this machine.".into(),
        ));
        return 0;
    }
    let literal: Vec<String> = named.iter().map(|path| literally(path)).collect();
    let mut add: Vec<&str> = vec!["add", "-A", "--"];
    add.extend(literal.iter().map(String::as_str));
    // **An exclude pathspec makes `git add -A` skip an untracked file named beside it** (git
    // 2.50, measured for #2772: `add -A -- :(literal)a/new :(exclude,glob)x/**` leaves `a/new`
    // untracked), so a new file a skill change names would miss its commit. A path inside what
    // the excludes keep out is never named ([`kept_out`]), and the excludes are passed only where
    // a named folder could hold it.
    let excluded = never_staged();
    if named.iter().any(|path| could_hold_kept_out(root, path)) {
        add.extend(excluded.iter().map(String::as_str));
    }
    save_staging(request, trigger, &claim, &add, say)
}

/// Whether `path` (plane-relative) is inside what a save never stages ([`never_staged`]): a
/// level's `skills.local/` by the one level rule, the installed skills' local lockfile, or a
/// throwaway piece.
fn kept_out(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    purlis_skills::in_local_level(path)
        || matches!(
            parts.as_slice(),
            ["skills.local.lock"] | ["workspaces", ".throwaway", ..]
        )
}

/// Whether `path` (plane-relative) is a folder that could hold what a save never stages: the
/// plane's root, `workspaces/` or `personas/`, or one workspace's or persona's folder.
fn could_hold_kept_out(root: &Path, path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    matches!(
        parts.as_slice(),
        [] | ["."] | ["workspaces" | "personas"] | ["workspaces" | "personas", _]
    ) && root.join(path).is_dir()
}

/// Whether `path` (plane-relative) can be named to `git add -A`: git tracks something under it,
/// or it is on disk and git does not ignore it. A name matching nothing would fail the add.
fn stageable(root: &Path, path: &str) -> bool {
    let spec = literally(path);
    let tracked = git::run(root, &["ls-files", "-z", "--", &spec], git::READ)
        .is_ok_and(|run| run.ok() && !run.out.is_empty());
    if tracked {
        return true;
    }
    if std::fs::symlink_metadata(root.join(path)).is_err() {
        return false;
    }
    // `check-ignore -q` exits 0 for an ignored path and 1 for one that is not.
    git::run(root, &["check-ignore", "-q", "--", path], git::READ)
        .is_ok_and(|run| run.code == Some(1))
}

/// `path` as a pathspec git reads letter for letter (`:(literal)`): a folder named `*` or `[a]`
/// names that folder and no other.
pub fn literally(path: &str) -> String {
    format!(":(literal){path}")
}

/// **Why a save of exactly `paths` would make no commit now**, or `None` where it would: the
/// project is one purlis may not write, has not been told how it is saved, saves nothing, or
/// already holds something staged outside `paths`, which the commit would carry. Asked before a
/// change that only a commit keeps (a skill's delete, #2770), and by [`save_paths`] itself.
pub fn paths_unsaved(root: &Path, paths: &[String]) -> Option<String> {
    if let Some(why) = crate::compat::read_only(root) {
        return Some(format!("nothing was written: {why}"));
    }
    match crate::planesave::Settings::read(root).plane.mode.value {
        None => {
            return Some(
                "Not saved: this project has not been told how it is saved yet, so the next \
                 save takes this change."
                    .to_owned(),
            );
        }
        Some(crate::planesave::Mode::Off) => {
            return Some("Not saved: this project's saving is off.".to_owned());
        }
        Some(_) => {}
    }
    let staged = git::run(root, &["diff", "--cached", "--name-only", "-z"], git::READ);
    let Some(staged) = staged.ok().filter(git::Run::ok) else {
        return Some(
            "Not saved: purlis could not ask git what is staged in this project.".to_owned(),
        );
    };
    let ours = |one: &str| {
        paths.iter().any(|path| {
            one == path
                || one
                    .strip_prefix(path.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    };
    staged
        .out
        .split('\0')
        .filter(|one| !one.is_empty())
        .any(|one| !ours(one))
        .then(|| OTHERS_STAGED.to_owned())
}

/// [`save_claimed`], staging with `add_cmd`.
fn save_staging(
    request: &Request,
    trigger: Trigger,
    _claim: &Claim,
    add_cmd: &[&str],
    say: Sink,
) -> u8 {
    let started = std::time::Instant::now();
    let plane = crate::planesave::Settings::read(request.root).plane;
    let mut attempt = Attempt::default();
    // The first thing the save refused with, in its own words, for a journal line that would
    // otherwise say only "failed".
    let mut refused: Option<String> = None;
    let mut heard = |line: Say| {
        if let Say::Fail(text) = &line
            && refused.is_none()
        {
            refused = Some(text.clone());
        }
        say(line);
    };
    let say: Sink = &mut heard;
    // A commit changes the committed tree, so a project this purlis may not write is not
    // saved, whoever asks: the window, the auto-save a chat's end pokes, or the command
    // (FR-24, #826). Nothing is staged, committed or pushed.
    let code = if let Some(why) = crate::compat::read_only(request.root) {
        say(Say::Fail(format!("nothing was written: {why}")));
        attempt.outcome = "blocked";
        1
    } else if plane.mode.value == Some(crate::planesave::Mode::Off) {
        say(Say::Info(format!(
            "[plane] mode is off ({}), so purlis commits nothing here — commit with git \
             yourself, or set another mode.",
            plane.mode.source.file().unwrap_or("default")
        )));
        attempt.outcome = "skipped";
        0
    } else {
        commit_push(
            request,
            &plane,
            add_cmd,
            &trigger.caller(),
            &mut attempt,
            say,
        )
    };
    if attempt.detail.is_empty()
        && let Some(refused) = refused
    {
        attempt.detail = refused;
    }
    let mode = match (request.no_push, plane.mode.value) {
        (true, Some(crate::planesave::Mode::Off)) | (false, _) => plane.mode.value,
        (true, _) => Some(crate::planesave::Mode::Commit),
    };
    journal_append(
        request.root,
        &serde_json::json!({
            "at": now(),
            "target": "plane",
            "trigger": trigger.word(),
            "mode": mode.unwrap_or(crate::planesave::Mode::Push).as_str(),
            "files": attempt.files,
            "commit": attempt.commit,
            "pr": attempt.pr,
            "ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "outcome": attempt.outcome,
            "detail": attempt.detail,
        }),
    );
    code
}

/// What one save attempt came to, filled in as it goes — the journal's line.
#[derive(Debug)]
struct Attempt {
    /// `saved`, `committed`, `pr-open`, `blocked`, `skipped` or `failed`.
    outcome: &'static str,
    files: usize,
    commit: Option<String>,
    pr: Option<String>,
    detail: String,
}

impl Default for Attempt {
    fn default() -> Self {
        Self {
            // Every path that is not a failure says what it was, so one that says nothing is.
            outcome: "failed",
            files: 0,
            commit: None,
            pr: None,
            detail: String::new(),
        }
    }
}

/// The most lines the journal keeps. The Saving view shows the newest fifty.
const JOURNAL_LINES: usize = 500;

/// Where the save journal lives: beside the push record, in the plane's state directory.
pub fn journal_path(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join("save-journal.jsonl")
}

/// The save journal, oldest first. A line that is not JSON is left out.
pub fn journal(root: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(journal_path(root))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Add `entry` to the journal, keeping the newest [`JOURNAL_LINES`]. Never fails loudly: a save
/// that cannot write its note must still have saved.
///
/// Read, then renamed over, with no lock: two saves finishing at the same instant — the app's
/// auto-save and a `charter save` — can lose one line between them. The file is never torn,
/// because the rename is atomic, and a lost line costs one row of the Saving view's history.
pub(crate) fn journal_append(root: &Path, entry: &serde_json::Value) {
    let path = journal_path(root);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let entry = entry.to_string();
    lines.push(&entry);
    let keep = &lines[lines.len().saturating_sub(JOURNAL_LINES)..];
    let mut out = keep.join("\n");
    out.push('\n');
    // As the push record is written: into the state directory `private_dir` has refused as a
    // link, whole, through `rewrite::replace`.
    if let Some(dir) = path.parent()
        && crate::profiletrust::private_dir(dir).is_ok()
    {
        let _ = crate::rewrite::replace(dir, &path, out.as_bytes(), crate::rewrite::Mode::Private);
    }
}

/// Seconds since the epoch, as the push record writes them.
pub(crate) fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Outcome three of three: charter could not determine the tree's state, so it stops.
///
/// **The refusal is the feature.** `charter save` returning 0 is what every caller reads as "it
/// is saved", and charter #917 is entirely made of what happens downstream of believing that.
/// The word "clean" appears nowhere on this path: the whole defect was a sentence containing
/// it.
fn refuse_unreadable(root: &Path, git_dir: &Path, doing: &str, said: &str, say: Sink) -> u8 {
    say(Say::Fail(format!(
        "Refusing to save — purlis could not read the state of {}, so it cannot tell an \
         unchanged tree from an unreadable one.",
        root.display()
    )));
    say(Say::Fail(format!(
        "  {doing}{}",
        if said.is_empty() {
            " without saying why".to_string()
        } else {
            format!(" — {said}")
        }
    )));
    match gitstate::find(git_dir) {
        Some(lock) => {
            say(Say::Info(format!("  {}", lock.describe())));
            for line in lock.remedy() {
                say(Say::Info(format!("  {line}")));
            }
        }
        // No lock, so charter has nothing to name and says so rather than inventing a remedy.
        None => say(Say::Info(format!(
            "  Nothing purlis can name is holding this index. `git -C {} status` is where the \
             reason will be.",
            root.display()
        ))),
    }
    1
}

/// How many directory rows the breakdown prints before it summarises the rest.
const BREAKDOWN_ROWS: usize = 12;

/// Say what is about to be committed, grouped by directory — the check the 2026-09-12 incident
/// says is the whole check.
fn show_what_is_staged(root: &Path, staged: &[String], say: Sink) {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in staged {
        let dir = match path.rsplit_once('/') {
            Some((dir, _)) => dir.to_string(),
            // A file at the top of the plane. Named rather than left under a bare `.`, because
            // a stray `uv.lock` at the root is exactly what this breakdown exists to surface.
            None => "(the plane root)".to_string(),
        };
        *counts.entry(dir).or_default() += 1;
    }
    say(Say::Warn(format!(
        "purlis save commits everything pending in {} — {} across {}, \
         not only what you meant:",
        root.display(),
        counted(staged.len(), "file", "files"),
        counted(counts.len(), "directory", "directories")
    )));
    // Biggest first, ties by name, so the row that explains the count is the first one read.
    let mut rows: Vec<(&String, &usize)> = counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    for &(dir, count) in rows.iter().take(BREAKDOWN_ROWS) {
        // Contained: a path comes out of the operator's tree, and a control character in one
        // would otherwise redraw the line it is printed on.
        say(Say::Info(format!(
            "  {count:>5}  {}",
            shown::readable(dir, shown::DISPLAY_LIMIT)
        )));
    }
    if rows.len() > BREAKDOWN_ROWS {
        say(Say::Info(format!(
            "  … and {} more directories. The whole list: git -C {} diff --cached --name-only",
            rows.len() - BREAKDOWN_ROWS,
            root.display()
        )));
    }
}

/// How many groups a generated message names before it says how many more there are.
const SUMMARY_GROUPS: usize = 4;

/// The message a save writes when it is given none: how many files, and what they are in the
/// plane's own words — `charter save: 3 files (steward memory 2, ide todos 1)` — biggest group
/// first, ties by name. `program` is the program as the plane spells it in its history
/// (D-RN11a-4): `charter` until rename-plane, `purlis` after.
fn summary(staged: &[String], program: &str) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in staged {
        *counts.entry(group_of(path)).or_default() += 1;
    }
    let mut groups: Vec<(String, usize)> = counts.into_iter().collect();
    groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut named: Vec<String> = groups
        .iter()
        .take(SUMMARY_GROUPS)
        .map(|(group, n)| format!("{group} {n}"))
        .collect();
    if groups.len() > SUMMARY_GROUPS {
        named.push(format!("+{} more", groups.len() - SUMMARY_GROUPS));
    }
    let files = if staged.len() == 1 { "file" } else { "files" };
    format!(
        "{program} save: {} {files} ({})",
        staged.len(),
        named.join(", ")
    )
}

/// What a plane path is part of, as a person would name it: a persona's memory, a
/// workspace's todos, the dispatch log — or, for anything else, its top directory.
fn group_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["personas", "_dispatch", ..] => "dispatch".into(),
        ["personas", "_skills", ..] => "skills".into(),
        ["personas", who, kind @ ("memory" | "refs"), _, ..] => format!("{who} {kind}"),
        ["personas", who, _, ..] => format!("{who} persona"),
        [
            "workspaces",
            ws,
            kind @ ("memory" | "todos" | "changes" | "pieces" | "refs"),
            _,
            ..,
        ] => {
            format!("{ws} {kind}")
        }
        ["workspaces", ws, _, ..] => format!("{ws} workspace"),
        [only] => (*only).to_string(),
        [top, ..] => (*top).to_string(),
        [] => String::new(),
    }
}

fn commit_push(
    request: &Request,
    plane: &crate::planesave::Plane,
    add_cmd: &[&str],
    caller: &forge::Caller,
    attempt: &mut Attempt,
    say: Sink,
) -> u8 {
    let root = request.root;
    // `--sign` asks for it this once; `[plane] sign` asks for it every time.
    let sign = request.sign || plane.sign.value;
    if stages_the_whole_tree(add_cmd) {
        // BEFORE the git-repo check and before any staging: a refusal that has already run
        // `git add -A` has done the damage and merely declined to name it.
        if let Some(tree) = tree_of(root, request.cwd) {
            let tree = tree.display();
            say(Say::Fail(format!(
                "Refusing to stage all of {} — you are standing in {tree}, a linked worktree of \
                 it. Committing every change in a tree you are not in would take that tree's \
                 uncommitted work under your message, and leave your own work here unsaved.",
                root.display()
            )));
            say(Say::Info(format!(
                "  your own work:   git -C {tree} add -A && git -C {tree} commit"
            )));
            say(Say::Info(format!(
                "  the plane's own: run `purlis save` from {}",
                root.display()
            )));
            say(Say::Info(format!(
                "  you really do mean this tree: PURLIS_ROOT={tree} CHARTER_ROOT={tree} purlis save"
            )));
            return 1;
        }
        // A SECOND ROUTE, not a widening of the first. A clone has a real `.git` directory, so
        // `tree_of` answers `None` here and must keep doing so. REFUSE rather than commit the
        // inner plane instead: committing the inner one would make `save` disagree with every
        // other command about which plane it acts on, and committing the outer one is the
        // defect. So charter names both trees and lets the caller say which they meant.
        if let Some(nested) = nested_plane_in(root, request.cwd) {
            let nested = nested.display();
            say(Say::Fail(format!(
                "Refusing to stage all of {} — you are standing in {nested}, a control plane of \
                 its own under that plane's workspaces/. purlis resolves outward to the plane \
                 holding the vault, so committing every change there would take that tree's \
                 uncommitted work under your message, and leave your own work here unsaved.",
                root.display()
            )));
            say(Say::Info(format!(
                "  your own work:   git -C {nested} add -A && git -C {nested} commit"
            )));
            say(Say::Info(format!(
                "  the plane's own: run `purlis save` from {}",
                root.display()
            )));
            say(Say::Info(format!(
                "  you really do mean this plane: PURLIS_ROOT={nested} CHARTER_ROOT={nested} \
                 purlis save"
            )));
            return 1;
        }
    }

    // A plane is not always a git repo: `charter init` in a fresh directory does not run `git
    // init`, and that is the README's own 60-second path. Every git call below tolerates a
    // non-zero exit, so without this the add failed silently, the probe answered "no
    // difference", and charter printed `✓ Committed : charter save: 0 file(s)` over a plane
    // with no history at all.
    let found = git::run(root, &["rev-parse", "--git-dir"], git::READ);
    let Some(found) = found.ok().filter(git::Run::ok) else {
        say(Say::Fail(format!(
            "{} is not a git repository, so there is nothing to commit to.",
            root.display()
        )));
        say(Say::Info(
            "  purlis init does not create one. Run: git init && git remote add origin <url>  \
             — personas and memory are meant to be committed and shared."
                .into(),
        ));
        return 1;
    };
    // Kept rather than thrown away with the return code it was asked for: it is where the
    // index lock lives, and for a linked worktree that is `<plane>/.git/worktrees/<name>`
    // rather than `<root>/.git` — the tree whose index this call is about.
    let named = found.line().trim();
    let git_dir = root.join(if named.is_empty() { ".git" } else { named });

    // Before anything is staged (#433): `git add -A` in a tree git has stopped part-way
    // through a merge or rebase stages the conflict markers as the resolution, and the commit
    // lands them. Refused whatever started the save — this command, the window's button,
    // auto-save — and the Saving view reads the same check as Blocked ([`standing`]).
    if let Some(stopped) = gitstate::stopped(root) {
        let why = stopped.why();
        say(Say::Fail(format!(
            "Not saved: in {}, {why}.",
            root.display()
        )));
        say(Say::Info(
            "  Nothing was staged or committed. Settle that, then save again.".into(),
        ));
        attempt.outcome = "blocked";
        attempt.detail = why;
        return 1;
    }

    // **The add's exit status is the whole fix.** Under a held `.git/index.lock` it exits 128
    // and stages nothing, and the probe below then answers 0 — because there is no difference
    // between HEAD and an index nothing was written to. Discarded, that made "charter could not
    // stage anything" and "there was nothing to stage" the same value.
    // The write that adds git's untracked cache to the index, which a status that can write it
    // (the operator's own) keeps fresh and every status then uses (FD-11, [`git::UNTRACKED_CACHE`]).
    let added = match git::run_untimed(root, &git::with_untracked_cache(root, add_cmd)) {
        Ok(run) => run,
        Err(unavailable) => {
            return refuse_unreadable(
                root,
                &git_dir,
                &format!("`git {}` could not be run", add_cmd.join(" ")),
                &unavailable.to_string(),
                say,
            );
        }
    };
    if !added.ok() {
        return refuse_unreadable(
            root,
            &git_dir,
            &format!(
                "`git {}` exited {}",
                add_cmd.join(" "),
                added
                    .code
                    .map_or_else(|| "None".to_string(), |c| c.to_string())
            ),
            &gitstate::said(&added),
            say,
        );
    }

    // Three outcomes, and `diff --quiet` has always answered in three: 0 for no difference, 1
    // for a difference, anything else for a failure. Read as `== 0` / else, the third collapsed
    // into the second and reached `git commit` with a message about zero files.
    let probe = match git::run(root, &["diff", "--cached", "--quiet"], git::READ) {
        Ok(run) => run,
        Err(unavailable) => {
            return refuse_unreadable(
                root,
                &git_dir,
                "`git diff --cached --quiet` could not be run",
                &unavailable.to_string(),
                say,
            );
        }
    };
    match probe.code {
        Some(0) => {
            // Commits a save of an explicit pushing mode left behind — pushed at quit past its
            // bound, or made by an agent with plain git — are carried on by the next save,
            // which is what a launch's save is for (ADR 0051). A plane that names no mode
            // keeps `charter save`'s recorded answer.
            // In a request mode, a known PR is settled by any save, so a clean `charter save` is how
            // a plane with auto-save off moves onto its merged PR.
            let pr_known = plane
                .mode
                .value
                .is_some_and(crate::planesave::Mode::opens_a_pr)
                && prsave::knows_a_pr(root);
            // The person's own press in the window carries a mode-less plane's commits too.
            let pressed = caller.surface() == forge::backend::Surface::Window;
            let unpushed = unpushed(root, plane, pressed);
            if !request.no_push && (unpushed.is_some() || pr_known) {
                say(Say::Info(match unpushed {
                    Some(n) => format!(
                        "Nothing new to commit — carrying on {} this machine has not \
                         saved yet.",
                        counted(n, "commit", "commits")
                    ),
                    None => format!(
                        "Nothing new to commit — asking after the open {}.",
                        forge::request_words_of(root, root).request_noun()
                    ),
                }));
                attempt.outcome = "committed";
                attempt.commit = git::run(root, &["rev-parse", "HEAD"], git::READ)
                    .ok()
                    .map(|r| r.line().trim().to_string())
                    .filter(|sha| !sha.is_empty());
                return carry(request, plane, sign, caller, attempt, say);
            }
            say(Say::Info(
                "Nothing to save — the control-plane working tree is clean.".into(),
            ));
            attempt.outcome = "skipped";
            return 0;
        }
        Some(1) => {}
        other => {
            return refuse_unreadable(
                root,
                &git_dir,
                &format!(
                    "`git diff --cached --quiet` exited {}",
                    other.map_or_else(|| "None".to_string(), |c| c.to_string())
                ),
                &gitstate::said(&probe),
                say,
            );
        }
    }
    // **`-z`, and the reason is the secret guard below** (M3). Without it git applies
    // `core.quotePath`, which is on by default: a staged `personas/café/memory/note.md`
    // comes back as `"personas/caf\303\251/memory/note.md"` — surrounding quotes and octal
    // escapes included. The guard still SELECTS that row — the substring test sees
    // `/memory/` — and then looks for a file under a name that does not exist, finds
    // nothing, and lets the row through. A path with a `"`, a backslash, a newline or any
    // byte over 0x7f walks past the guard that way. `-z` turns the quoting off and NUL is
    // then the only separator, which is also the only one a filename cannot hold.
    let staged: Vec<String> = git::run(root, &["diff", "--cached", "--name-only", "-z"], git::READ)
        .map(|r| {
            r.out
                .split('\0')
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    attempt.files = staged.len();
    show_what_is_staged(root, &staged, say);

    // The secret guard: refuse if any staged file looks like it holds a secret. Every file a
    // save stages is committed and pushed to a shared repository, so a credential in any of
    // them is disclosed the moment the save lands.
    //
    // **It reads the STAGED BLOB, which is the thing about to be committed** (M3). Reading
    // the working-tree file instead asked about bytes that need not be the bytes of the
    // commit, and the gap is not theoretical — measured with git 2.50.1:
    //
    //     git add <memory file with the secret in it>
    //     git update-index --skip-worktree <that file>     # `add -A` now skips it
    //     printf 'clean\n' > <that file>
    //
    // leaves the secret in the index, `clean` on disk, the row still listed by
    // `diff --cached --name-only`, and the guard reading `clean`. `git show :<path>` asks
    // the index instead, so there is no second copy of the file to disagree with. It also
    // answers for a row with no file on disk at all, which the working-tree read could only
    // skip.
    // A path the save DELETES carries no content to disclose, and has no staged blob to read:
    // asked for one, the guard below would refuse every save that removes a memory — which is
    // what going LOCAL does to a workspace's (charter-app#301).
    let deleted: std::collections::HashSet<String> = git::run(
        root,
        &["diff", "--cached", "--name-only", "-z", "--diff-filter=D"],
        git::READ,
    )
    .map(|r| {
        r.out
            .split('\0')
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default();
    // Where each finding is: the line a credential starts on, or none for a file refused
    // whole.
    let mut flagged: Vec<(String, Option<usize>, &'static str)> = Vec::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();
    let mut to_read: Vec<&String> = Vec::new();
    for path in &staged {
        if deleted.contains(path) {
            continue;
        }
        let file = root.join(path);
        // What is committed for a link outside memory and refs is the link, a path and no
        // text of its target, and a directory is a nested repository's gitlink, a commit id:
        // neither has a blob of text to scan. A memory or ref link goes on to be refused.
        if !memory_or_ref(path)
            && std::fs::symlink_metadata(&file)
                .is_ok_and(|found| found.file_type().is_symlink() || found.is_dir())
        {
            continue;
        }
        // charter #442's shape, kept and asked first so its own sentence is the one a
        // committed `memory/x -> /etc/passwd` gets: charter will not read through a link
        // that leaves the plane, and a memory file it cannot read is not one it commits.
        if !crate::contain::within_plane(root, &file) {
            flagged.push((
                path.clone(),
                None,
                "it leads out of the plane, so purlis will not read it",
            ));
            continue;
        }
        // A link that lands back INSIDE the plane is refused too, and the blob is why: what
        // a save commits for a link is the link — a blob holding the target's path — so
        // scanning that blob would answer "no secret" about a file charter never read.
        // Following it instead would be a guard whose answer is about bytes the commit does
        // not carry, which is the whole defect this loop was changed to close.
        if std::fs::symlink_metadata(&file).is_ok_and(|found| found.file_type().is_symlink()) {
            flagged.push((
                path.clone(),
                None,
                "it is a link, and what a save commits for one is the link rather than the \
                 text purlis would have read",
            ));
            continue;
        }
        to_read.push(path);
    }
    for (path, blob) in to_read.iter().zip(staged_blobs(root, &to_read)) {
        match blob {
            Some(text) => {
                if let Some(why) = unscanned(path, text.as_bytes()) {
                    skipped.push(((*path).clone(), why));
                } else if let Some((line, kind)) = secret_in(path, &text) {
                    flagged.push(((*path).clone(), line, kind));
                }
            }
            // A row charter could not read what is staged for is not one it commits
            // unexamined — the same rule as the link above, one source along. Before this,
            // an unreadable row fell through to no flag at all, which is the one direction
            // a guard may not fail in.
            //
            // **No test reaches this arm, and that is measured rather than assumed.** The
            // obvious way to make one — `update-index --cacheinfo` with a sha that is not
            // in the object store — does not survive the `add -A` this command runs first:
            // git stages the DELETION of a path with no file on disk — a row the guard now
            // skips before it gets here, because a deletion publishes nothing (it did reach
            // here until charter-app#301, and refused every save that removed a memory). What
            // is left that reaches here is a `git show` that fails for
            // a reason a fixture cannot manufacture: a corrupt object store, the deadline,
            // git gone from PATH mid-command. Deleting this arm therefore turns nothing
            // red, and the next person to mutate it should know that before they conclude
            // it is dead code.
            None => flagged.push((
                (*path).clone(),
                None,
                "purlis could not read what is staged for it, so it will not commit it \
                 unexamined",
            )),
        }
    }
    if !skipped.is_empty() {
        say(Say::Info(
            "Not scanned for secrets — git calls it binary, or it is over 10 MiB:".into(),
        ));
        for (path, why) in &skipped {
            say(Say::Info(format!(
                "  {} ({why})",
                shown::readable(path, shown::DISPLAY_LIMIT)
            )));
        }
    }
    if !flagged.is_empty() {
        say(Say::Fail(
            "Refusing to save — a secret-shaped value in a staged file:".into(),
        ));
        for (path, line, kind) in &flagged {
            let at = line.map(|line| format!(":{line}")).unwrap_or_default();
            say(Say::Fail(format!(
                "  {}{at}  ({kind})",
                shown::readable(path, shown::DISPLAY_LIMIT)
            )));
        }
        say(Say::Info(
            "Secrets belong in the vault (`purlis persona secret set`). Remove it, then retry."
                .into(),
        ));
        attempt.outcome = "blocked";
        attempt.detail = SECRET_REFUSED.into();
        return 1;
    }

    let msg = match request.message {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => summary(&staged, crate::names::BINARY.writes_for(root)),
    };
    // The agent run's trailers (GL-8, V67), appended by charter itself and never through git's
    // `--trailer`, which reads trailer configuration and reformats the message's own lines.
    let msg = match request.provenance {
        Some(p) => crate::provenance::append(&msg, &p.trailers(plane.assisted_by.value), None),
        None => msg,
    };
    // Unsigned by default so a signer never hangs; `--sign` opts in. `-c` on the command line
    // beats the operator's global `commit.gpgsign = true`, which is the whole point: their
    // preference keeps working everywhere else on their machine, and inside the plane the
    // command-line value wins.
    let mut commit: Vec<&str> = vec!["-c", gpgsign(sign)];
    commit.extend(["commit", "-q", "-m", msg.as_str()]);
    let committed = git::run(root, &commit, WRITE);
    let still_staged = |root: &Path| {
        git::run(root, &["diff", "--cached", "--quiet"], git::READ).is_ok_and(|r| !r.ok())
    };
    if sign && still_staged(root) {
        // Asked to sign — `--sign`, or `[plane] sign` — and the signer refused. There is no
        // unsigned retry: an unsigned commit is exactly what the operator asked charter not to
        // make, and the next push would carry it to the remote (ADR 0051).
        let signer = committed
            .map(|run| signer_said(&run.err))
            .unwrap_or_default();
        let why = if signer.is_empty() {
            "purlis could not sign the commit".to_string()
        } else {
            format!("purlis could not sign the commit: {signer}")
        };
        say(Say::Fail(format!(
            "{why} — {} staged but not committed.",
            files_are(staged.len())
        )));
        say(Say::Info(
            "  Fix the signer, or set sign = false under [plane], then save again.".into(),
        ));
        attempt.detail = why;
        return 1;
    }
    // Still staged = nothing was committed. Reporting success here is how a
    // failed commit became `✓ Committed :` with an empty sha — the sha was empty precisely
    // BECAUSE there was no commit, and that was the only visible symptom.
    if still_staged(root) {
        say(Say::Fail(format!(
            "git commit failed — {} staged but not committed.",
            files_are(staged.len())
        )));
        say(Say::Info(format!(
            "  Run `git -C {} status` to see why; purlis has left them staged.",
            root.display()
        )));
        return 1;
    }
    attempt.outcome = "committed";
    attempt.commit = git::run(root, &["rev-parse", "HEAD"], git::READ)
        .ok()
        .map(|r| r.line().trim().to_string())
        .filter(|sha| !sha.is_empty());
    let short = git::run(root, &["rev-parse", "--short", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    // Names the TREE. charter #806 lived for months because it did not: a commit landing in
    // the plane's clone while the operator stood in a worktree read exactly like a commit
    // landing where they meant.
    say(Say::Done(format!(
        "Committed {short} in {}: {msg}  ({})",
        root.display(),
        counted(staged.len(), "file", "files")
    )));

    carry(request, plane, sign, caller, attempt, say)
}

/// How many commits a clean plane has that its mode would still push: `None` unless the plane
/// names a mode past `commit` and the target branch on the remote lacks commits HEAD has —
/// and, in a request mode, the pull request does not already carry HEAD. A plane that names no
/// mode counts too when the window's Save was `pressed`, or while its last push stopped and
/// has not landed ([`retries_its_push`]): the window never answers "nothing to save" while the
/// remote lacks commits HEAD has (qw382).
/// `1 file is`, `3 files are`: what a failed commit left staged.
pub(crate) fn files_are(n: usize) -> String {
    format!("{} {}", counted(n, "file", "files"), plural(n, "is", "are"))
}

fn unpushed(root: &Path, plane: &crate::planesave::Plane, pressed: bool) -> Option<u32> {
    use crate::planesave::Mode;
    let mode = match plane.mode.value {
        Some(mode @ (Mode::Push | Mode::Pr | Mode::PrMerge)) => mode,
        None if pressed || retries_its_push(root) => Mode::Push,
        _ => return None,
    };
    let branch = match &plane.branch.value {
        Some(branch) => branch.clone(),
        None => git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
            .ok()?
            .line()
            .trim()
            .to_string(),
    };
    let ahead = count(root, &format!("refs/remotes/origin/{branch}..HEAD"))?;
    if ahead == 0 {
        return None;
    }
    if mode.opens_a_pr()
        && let Some(rec) = unlanded(root)
        && rec.get("outcome").and_then(serde_json::Value::as_str) == Some("pr-open")
    {
        let head = git::run(root, &["rev-parse", "HEAD"], git::READ)
            .map(|r| r.line().trim().to_string())
            .unwrap_or_default();
        if rec.get("head").and_then(serde_json::Value::as_str) == Some(head.as_str()) {
            return None;
        }
    }
    Some(ahead)
}

/// Take a plane whose work is committed as far as its mode goes: nowhere, a push to the target
/// branch, or the save branch and its pull request.
fn carry(
    request: &Request,
    plane: &crate::planesave::Plane,
    sign: bool,
    caller: &forge::Caller,
    attempt: &mut Attempt,
    say: Sink,
) -> u8 {
    let root = request.root;
    if request.no_push {
        say(Say::Info("Skipped push (--no-push).".into()));
        return 0;
    }
    let file = plane.mode.source.file().unwrap_or("default");
    let key = if plane.from_share {
        "[memory] share"
    } else {
        "[plane] mode"
    };
    if plane.mode.value == Some(crate::planesave::Mode::Commit) {
        say(Say::Info(format!("Not pushed: {key} is commit ({file}).")));
        return 0;
    }
    let pr_mode = plane
        .mode
        .value
        .is_some_and(crate::planesave::Mode::opens_a_pr);
    if !pr_mode && origin_https(root).is_none() {
        say(Say::Warn(
            "origin isn't on a forge purlis knows (gitlab.com/github.com/…) — committed \
             locally; push manually."
                .into(),
        ));
        return 0;
    }
    // rc 1 only when the commit reached NOWHERE — a pull-request branch that also failed. An
    // ordinary push failure has been reported and stays rc 0: `charter save` having committed
    // successfully is not a failed command.
    let target = plane.branch.value.as_deref();
    if let Some(target) = target {
        // Pushing HEAD to a branch it is not on would rebase THIS branch onto that one the
        // moment the remote moved — replaying its history there and rewriting it here. Nothing
        // rewrites anyone's history (ADR 0051): the commit stays, and the operator says which
        // branch they meant.
        let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
            .map(|r| r.line().trim().to_string())
            .unwrap_or_default();
        if here != target {
            let why = format!("this plane is on {here}, and [plane] branch is {target}");
            say(Say::Warn(format!(
                "Not pushed: {why} ({}).",
                plane.branch.source.file().unwrap_or("default")
            )));
            say(Say::Info(format!(
                "  Check out {target}, or change [plane] branch, then save again."
            )));
            attempt.outcome = "blocked";
            attempt.detail = why;
            return 0;
        }
    }
    let pushed = if pr_mode {
        push_for_a_request(root, plane, sign, caller, say)
    } else {
        push_head(root, target, sign, say)
    };
    // The push may have rebased: the commit that landed is HEAD now.
    attempt.commit = git::run(root, &["rev-parse", "HEAD"], git::READ)
        .ok()
        .map(|r| r.line().trim().to_string())
        .filter(|sha| !sha.is_empty());
    attempt.outcome = match pushed.outcome {
        Outcome::Pushed => "saved",
        Outcome::Branched | Outcome::PrOpen => "pr-open",
        Outcome::Unreachable => "committed",
        Outcome::Conflict | Outcome::Blocked => "blocked",
        Outcome::Stranded | Outcome::Failed => "failed",
    };
    attempt.pr = pushed.url.clone();
    attempt.detail = pushed.detail.clone();
    u8::from(pushed.outcome == Outcome::Stranded)
}

/// Push what a save of the plane at `root` committed, as its mode says: to the target branch,
/// or — in a request mode — to the save branch, with its pull request opened or updated. What
/// quitting runs once its commit is made ([`crate::autosave::at_quit`]).
pub fn push_saved(root: &Path, sign: bool, say: Sink) -> PushResult {
    let plane = crate::planesave::Settings::read(root).plane;
    if plane
        .mode
        .value
        .is_some_and(crate::planesave::Mode::opens_a_pr)
    {
        push_for_a_request(root, &plane, sign, &forge::Caller::trigger(), say)
    } else {
        push_head(root, plane.branch.value.as_deref(), sign, say)
    }
}

mod blockedby;
pub mod merging;
mod prsave;
pub mod resolve;
mod update;
pub use merging::{MergeState, Merging, Seen, changed_since};
pub use prsave::{MergedAsItStands, settled_at, stamp_is_unusable};
pub use update::{BY_HAND, Left, Moved, Updated, update};

pub use blockedby::{SecretAt, file_of as blocked_file};

#[cfg(test)]
mod tests;
