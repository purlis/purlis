// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The plane's `pr` and `pr-merge` modes (charter-app#298, ADR 0051): one rolling save branch
//! per machine, and one pull request from it into the target branch.
//!
//! # What a save does
//!
//! The commit is made on the target branch, as every mode makes it. Then:
//!
//! 1. **The last PR is settled** ([`settle`]): if it has merged since, the local target branch
//!    is moved onto the remote's, and if it was closed without merging, the plane is blocked.
//! 2. **HEAD is pushed to the save branch** (`[plane] save_branch`, default
//!    `purlis/save/<host>-<this clone>`, or the `charter/save/…` one a request still open from
//!    before the rename is on: [`save_branch`]), with `--force-with-lease` against the commit
//!    this clone last pushed there, which `save-branch.json` keeps. A force is needed because a
//!    squash or rebase merge leaves the save branch's old tip out of the target's history, and
//!    the lease is what keeps it from overwriting anything somebody else pushed there — a
//!    second machine with the same name included. With nothing kept, the branch is leased as
//!    absent, and one that exists is pushed over only when its tip is already in HEAD's
//!    history. **Only the save branch is ever forced**: the target branch is never pushed to.
//! 3. **The PR is opened or updated** into the target branch, named explicitly
//!    ([`crate::forge::pr::open_or_update`]). In `pr-merge` purlis then merges it itself, at
//!    the commit just pushed, once it is ready ([`merge_it_ourselves`]) — and never at the
//!    save: a commit pushed a moment ago has no check yet, and "no check yet" is not "no
//!    check" (ADR 0051, amended 2026-10-13). The save records that it waits; [`settle`] asks
//!    on every save and every fetch, and the app looks again by itself ([`super::look`]),
//!    until every check at that commit has passed, or the commit has been there long enough
//!    and nothing says a check will run. A failed check of any kind, required or not, leaves
//!    it open and unmerged, as does a review the branch requires. The forge's own auto-merge
//!    is never asked for: it merges past a failing check that is not required.
//! 4. **The outcome is recorded** in `plane-push.json` as `pr-open`, with the PR's link and
//!    number, so [`super::standing`] can say *pushed, PR open* without asking the network.
//!    The PR itself — number, link, the commit it was last pushed at and when, and where it
//!    stands with merging ([`Merging`]) — is kept apart, in `save-branch.json` ([`Kept`]),
//!    which no failure overwrites: a merged PR is settled even when the saves after it failed,
//!    and never proposed a second time.
//!
//! # After the PR merges
//!
//! Merged is the forge's word, asked by [`settle`] on every save and every fetch while a PR is
//! known. A save whose question the forge cannot answer stops before it pushes. The target
//! branch is fetched, and it holds this machine's work when the pushed commit is in its
//! history (a merge commit), or when the commit the forge names as the merge — a squash, or a
//! rebase's last commit — is in its history and matches the pushed tree on every path the PR
//! touched. It is that commit and not the target's tip that is compared, so an edit somebody
//! made to one of those paths after the merge does not count against it. Then the local branch
//! is moved onto the remote's: `reset --keep` when nothing was committed since the push, which
//! keeps uncommitted work and refuses rather than overwrite it — the next look tries again —
//! or a rebase of the newer commits onto it. A merge commit that does not hold the work blocks
//! the plane, and nothing is moved.

use super::*;
use crate::counts::counted;
use crate::forge::Caller;
use crate::forge::pr::{self, Gate, Pinned, Pr, Ready, Repo, State};
use crate::planesave::{Mode, Plane};

/// Whether a push record is one a request mode wrote: its present tense is asked of the target
/// branch and HEAD ([`still_holds`]) rather than of the branch's upstream.
pub(super) fn is_pr_record(rec: &serde_json::Value) -> bool {
    matches!(
        rec.get("outcome").and_then(serde_json::Value::as_str),
        Some("pr-open" | "blocked")
    )
}

/// Whether a request mode's record is still true: the commit it is about has not reached the
/// remote's target branch, and the plane is still on it or on a commit made after it.
///
/// The second half is what lets a person clear a block by hand: a plane moved off the
/// recorded commit — reset onto the remote's branch, say — is no longer the plane the record
/// is about. So is one whose save_branch was changed away from the branch the record names.
pub(super) fn still_holds(root: &Path, rec: &serde_json::Value, head: &str) -> bool {
    let Some(branch) = rec
        .get("branch")
        .and_then(serde_json::Value::as_str)
        .filter(|b| crate::planesave::branch_ok(b))
    else {
        return false;
    };
    // A block about a save branch the plane no longer pushes to — its save_branch changed, by
    // hand, to get past another clone's commits — is not about this plane any more.
    let landed = rec.get("landed").and_then(serde_json::Value::as_str);
    let plane = crate::planesave::Settings::read(root).plane;
    // This clone's own default under any prefix it has had is still this plane's: a block
    // about the request a pre-rename save opened from `charter/save/…` holds until the next
    // save, as it did before the rename (V93j).
    if landed.is_some_and(|landed| {
        landed != save_branch(root, &plane) && !is_own_default(root, &plane, landed)
    }) {
        return false;
    }
    is_object_name(head)
        && !is_ancestor(root, head, &format!("refs/remotes/origin/{branch}"))
        && is_ancestor(root, head, "HEAD")
}

/// `git merge-base --is-ancestor`: only a clean yes counts.
fn is_ancestor(root: &Path, commit: &str, of: &str) -> bool {
    git::run(
        root,
        &["merge-base", "--is-ancestor", commit, of],
        git::READ,
    )
    .is_ok_and(|r| r.ok())
}

/// The branch HEAD is on, or an empty string.
fn here(root: &Path) -> String {
    git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default()
}

/// The object name `rev` resolves to, or `None`.
fn resolve(root: &Path, rev: &str) -> Option<String> {
    git::run(root, &["rev-parse", "--verify", "-q", rev], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|sha| !sha.is_empty())
}

/// The save branch this clone pushes to: [`Plane::save_branch_or_default`], except that a clone
/// whose pull request is still open from its default save branch under an older prefix —
/// `charter/save/<host>-<clone>`, from before the rename — carries on there until that request
/// merges or closes, so the rename never opens a second one beside it (V93j). Once it is
/// settled, the next push goes to the purlis name. A `save_branch` set by hand is always used
/// as it is.
///
/// "Still open" is a request kept, or one [`Kept::held`]: a plane moved off the request's
/// commit by hand forgets the request while it may still be open on the forge, and pushing
/// to the purlis name then would open a second one beside it. Only a settle that SAW it merged
/// or closed ends that ([`forget_old_branch`]). A kept old branch with no request — a clone
/// upgraded after its last one merged — goes straight to the purlis name.
pub(super) fn save_branch(root: &Path, plane: &Plane) -> String {
    let fresh = plane.save_branch_or_default(root);
    let kept = Kept::read(root);
    match kept.branch {
        Some(branch)
            if (kept.pr.is_some() || kept.held)
                && branch != fresh
                && is_own_default(root, plane, &branch) =>
        {
            branch
        }
        _ => fresh,
    }
}

/// Whether `branch` is this clone's default save branch under any prefix the product has used,
/// for a plane that names no save_branch of its own.
fn is_own_default(root: &Path, plane: &Plane, branch: &str) -> bool {
    plane.save_branch.value.is_none()
        && crate::names::BRANCH_PREFIX.strip(branch)
            == Some(crate::planesave::default_rest(root).as_str())
}

/// After a settle saw the request merged or closed: a pre-rename save branch has carried its
/// last request, so what is kept stops naming it and the next push goes to the purlis name.
fn forget_old_branch(root: &Path, plane: &Plane, kept: &mut Kept) {
    if kept
        .branch
        .as_deref()
        .is_some_and(|b| b != plane.save_branch_or_default(root) && is_own_default(root, plane, b))
    {
        *kept = Kept::default();
    }
}

/// What a request mode needs before it can push anything: the forge repo the plane's origin names,
/// the save branch, and the target branch. `Err` is a config error, in words, and the plane is
/// blocked on it (ADR 0051).
pub(super) fn config(root: &Path, plane: &Plane) -> Result<(Repo, String, String), String> {
    let repo = Repo::of_plane(root)?;
    let save = save_branch(root, plane);
    let target = plane.branch.value.clone().unwrap_or_else(|| here(root));
    if save == target {
        return Err(format!(
            "[plane] save_branch is {save}, the target branch itself, and a request mode never \
             pushes to the branch its {} goes into. Name another save_branch, or \
             remove it for {}save/<host>-<clone>",
            repo.forge.kind.request_noun(),
            crate::names::BRANCH_PREFIX.write
        ));
    }
    Ok((repo, save, target))
}

/// What this clone keeps about its save branch: the commit it last pushed there, which the
/// next push is leased against, and the PR it last opened from there. `save-branch.json`, in
/// the plane's state directory, beside `plane-push.json` — and apart from it, because that
/// record is rewritten by every push, failures included.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Kept {
    /// The save branch the rest is about.
    pub branch: Option<String>,
    /// The commit this clone last pushed to it.
    pub pushed: Option<String>,
    pub pr: Option<KeptPr>,
    /// The request [`Self::pr`] named was forgotten without being seen to merge or close — the
    /// plane was moved off its commit by hand — so it may still be open, and a pre-rename save
    /// branch carries on until a settle sees it end ([`save_branch`], V93j).
    pub held: bool,
}

/// The PR a save last opened or updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KeptPr {
    pub number: u64,
    pub url: String,
    /// The commit it was last pushed at.
    pub head: String,
    /// The branch it goes into.
    pub target: String,
    /// purlis merges it itself once it is ready ([`merge_it_ourselves`]): every `pr-merge`
    /// save's request, until it is in a merge queue or train. `merges_itself` in the file,
    /// there only when true.
    pub merges_itself: bool,
    /// When [`Self::head`] was pushed, in seconds since the epoch: what the wait for its
    /// checks to start is measured from ([`settled`]). `None` in a file from before this was
    /// kept, whose request has been open long enough.
    pub pushed_at: Option<u64>,
    /// Where it stands with merging, as the last look found it.
    pub merge: Option<Merging>,
}

/// Whether a commit pushed at `pushed_at` has been on the forge long enough for its checks to
/// have started ([`pr::SETTLE`]), at `now`. A stamp from a clock that has since gone back by
/// more than the wait says nothing, and is read as long ago rather than as never.
pub fn settled_at(pushed_at: Option<u64>, now: u64) -> bool {
    pushed_at.is_some_and(|at| now >= at.saturating_add(pr::SETTLE.as_secs()))
}

/// Whether a stamp says nothing a look can use: there is none, or it is in the future.
///
/// Then the look that finds it stamps the request again, with now ([`restamp`]), so it
/// settles a wait later rather than never: a clock set back, or a file from before the stamp
/// was kept.
pub fn stamp_is_unusable(pushed_at: Option<u64>, now: u64) -> bool {
    pushed_at.is_none_or(|at| at > now)
}

/// Stamp the kept request as pushed now. A stamp can only ever be moved later this way.
fn restamp(root: &Path) {
    let mut kept = Kept::read(root);
    if let Some(pr) = kept.pr.as_mut() {
        pr.pushed_at = Some(now_secs());
        kept.write(root);
    }
}

/// [`settled_at`], now.
fn settled(pushed_at: Option<u64>) -> bool {
    settled_at(pushed_at, now_secs())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Where [`Kept`] lives.
pub fn kept_path(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join("save-branch.json")
}

impl Kept {
    /// What is kept, with anything not in the shape charter writes left out: each value ends up
    /// in git's argv or a forge's API path.
    pub(super) fn read(root: &Path) -> Kept {
        let path = kept_path(root);
        let Some(doc) = read_json(&path) else {
            return Kept::default();
        };
        let text = |v: &serde_json::Value, key: &str| {
            v.get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        let pr = doc.get("pr").and_then(|pr| {
            Some(KeptPr {
                number: pr.get("number").and_then(serde_json::Value::as_u64)?,
                url: text(pr, "url").unwrap_or_default(),
                head: text(pr, "head").filter(|h| is_object_name(h))?,
                target: text(pr, "target").filter(|b| crate::planesave::branch_ok(b))?,
                merges_itself: pr.get("merges_itself").and_then(serde_json::Value::as_bool)
                    == Some(true),
                pushed_at: pr.get("pushed_at").and_then(serde_json::Value::as_u64),
                merge: pr.get("merge").and_then(Merging::from_json),
            })
        });
        Kept {
            branch: text(&doc, "branch").filter(|b| crate::planesave::branch_ok(b)),
            pushed: text(&doc, "pushed").filter(|h| is_object_name(h)),
            pr,
            held: doc.get("held").and_then(serde_json::Value::as_bool) == Some(true),
        }
    }

    /// Write it down. Never fails loudly, as the push record does not.
    pub(super) fn write(&self, root: &Path) {
        let path = kept_path(root);
        let doc = serde_json::json!({
            "branch": self.branch,
            "pushed": self.pushed,
            "pr": self.pr.as_ref().map(|pr| {
                let mut kept = serde_json::json!({
                    "number": pr.number,
                    "url": pr.url,
                    "head": pr.head,
                    "target": pr.target,
                });
                if pr.merges_itself {
                    kept["merges_itself"] = serde_json::Value::Bool(true);
                }
                if let Some(at) = pr.pushed_at {
                    kept["pushed_at"] = serde_json::json!(at);
                }
                if let Some(merge) = &pr.merge {
                    kept["merge"] = merge.to_json();
                }
                kept
            }),
        });
        let mut doc = doc;
        if self.held {
            doc["held"] = serde_json::Value::Bool(true);
        }
        if let Some(dir) = path.parent()
            && crate::profiletrust::private_dir(dir).is_ok()
        {
            let _ = crate::rewrite::replace(
                dir,
                &path,
                crate::pyjson::dumps_indent2(&doc).as_bytes(),
                crate::rewrite::Mode::Private,
            );
        }
    }
}

/// The link of the PR this clone last opened, while it is known.
pub(super) fn known_pr_url(root: &Path) -> Option<String> {
    Kept::read(root)
        .pr
        .map(|pr| pr.url)
        .filter(|u| !u.is_empty())
}

/// Whether a PR this clone opened is known and not yet settled.
pub(super) fn knows_a_pr(root: &Path) -> bool {
    Kept::read(root).pr.is_some()
}

/// Where the PR this clone last opened stands with merging, while it is known.
pub(super) fn known_merging(root: &Path) -> Option<Merging> {
    Kept::read(root).pr.and_then(|pr| pr.merge)
}

/// The commit the kept request is at, and where it stands with merging, when one is kept.
pub(super) fn kept_seen(root: &Path) -> Option<(String, Option<Merging>)> {
    Kept::read(root).pr.map(|pr| (pr.head, pr.merge))
}

/// The request this clone keeps open, when one is: when its head was pushed, and its head.
pub(super) fn open_request(root: &Path) -> Option<(Option<u64>, String)> {
    Kept::read(root).pr.map(|pr| (pr.pushed_at, pr.head))
}

/// Keep `merging` as where the request stands, when it is not what is kept already. Answers
/// whether it changed.
fn remember(root: &Path, merging: &Merging, queued: bool) -> bool {
    let mut kept = Kept::read(root);
    let Some(pr) = kept.pr.as_mut() else {
        return false;
    };
    let changed = pr.merge.as_ref() != Some(merging) || (queued && pr.merges_itself);
    if changed {
        pr.merge = Some(merging.clone());
        if queued {
            // In the forge's queue now: nothing more is asked of it about merging this one.
            pr.merges_itself = false;
        }
        kept.write(root);
    }
    changed
}

/// Push the plane's HEAD to the save branch and open or update its pull request — the PR
/// modes' half of a save, once the commit is made. A push record it could not write is said.
///
/// `caller` is who the forge is asked as: the person in the window for the save button, whose
/// purlis sign-in then opens the pull request (#1816).
pub(super) fn push(
    root: &Path,
    plane: &Plane,
    sign: bool,
    caller: &Caller,
    say: Sink,
) -> PushResult {
    let res = push_unsaid(root, plane, sign, caller, say);
    super::said_if_unrecorded(res, say)
}

/// [`push`], before what it could not record is said.
fn push_unsaid(root: &Path, plane: &Plane, sign: bool, caller: &Caller, say: Sink) -> PushResult {
    let mode = plane.mode.value.unwrap_or(Mode::Pr);
    let refused = |why: String, say: Sink| {
        say(Say::Warn(format!("Not pushed: {why}.").into()));
        // Unrecorded: a config error is re-derived by `standing` from the settings, and
        // recording it could only outlive the fix.
        PushResult {
            detail: why,
            ..PushResult::of(Outcome::Blocked, "")
        }
    };
    let Some(https) = origin_https(root) else {
        // Not a block (charter-app#295): a request mode on a remote no forge adapter serves commits
        // and goes no further, and `standing` says so as a notice.
        say(Say::Warn(
            "Not pushed: this plane's origin is not a GitHub or GitLab forge purlis knows, so \
             there is nowhere to open a pull request — committed only."
                .into(),
        ));
        return PushResult::of(Outcome::Unreachable, "");
    };
    let (repo, save, target) = match config(root, plane) {
        Ok(found) => found,
        Err(why) => return refused(why, say),
    };
    let here = here(root);
    if here != target {
        return refused(
            format!("this plane is on {here}, and [plane] branch is {target}"),
            say,
        );
    }
    // The window's own sign-in goes only to the remote recorded for this plane (#752).
    let helper = match forge::gitcredential::helper_for_push(&repo.forge, root, &https) {
        Ok(helper) => helper,
        Err(why) => return refused(why.to_string(), say),
    };
    let kind = repo.forge.kind;
    let cli = kind.cli();
    let noun = kind.request_noun();

    let head = resolve(root, "HEAD").unwrap_or_default();
    let settled = settle(
        root,
        plane,
        &target,
        sign,
        Some((&https, &helper)),
        caller,
        say,
    );
    let head = match settled {
        Settled::Blocked(res) => return res,
        Settled::Unknown(why) => {
            say(Say::Warn(
                format!(
                    "Committed, but not pushed: {why}. Nothing is pushed until purlis knows \
                 whether the last {noun} merged; the next save asks again."
                )
                .into(),
            ));
            return record_push(
                root,
                PushResult {
                    detail: why,
                    ..PushResult::of(Outcome::Failed, &target)
                },
                &head,
            );
        }
        Settled::Waiting(why) => {
            say(Say::Info(
                format!("Committed, not pushed yet: {why}").into(),
            ));
            // Unrecorded, as nothing went wrong: the next look tries again.
            return PushResult {
                detail: why,
                ..PushResult::of(Outcome::Unreachable, &target)
            };
        }
        // A move rewrote HEAD.
        Settled::Moved => resolve(root, "HEAD").unwrap_or_default(),
        Settled::Nothing | Settled::Open => head,
    };
    // A merged request was the last one a pre-rename save branch carries: what follows it goes
    // to the purlis name.
    let save = if settled == Settled::Moved {
        save_branch(root, plane)
    } else {
        save
    };
    if count(root, &format!("refs/remotes/origin/{target}..HEAD")) == Some(0) {
        say(Say::Done(
            format!("Nothing left to push — {target} on the remote already has all of it.").into(),
        ));
        let _ = std::fs::remove_file(push_record_path(root));
        return PushResult::of(Outcome::Pushed, &target);
    }
    let mut kept = Kept::read(root);
    if kept.branch.as_deref() != Some(save.as_str()) {
        // Kept about another save branch: nothing of it applies to this one.
        kept = Kept {
            branch: Some(save.clone()),
            ..Kept::default()
        };
    }
    if settled == Settled::Open
        && let Some(open) = kept.pr.as_ref().filter(|pr| pr.head == head)
        && kept.pushed.as_deref() == Some(head.as_str())
    {
        say(Say::Done(
            format!(
                "{} {} already carries all of it: {}",
                kind.request_noun_capitalised(),
                kind.request_ref(open.number),
                open.url
            )
            .into(),
        ));
        return record_push(
            root,
            PushResult {
                landed: Some(save),
                url: Some(open.url.clone()),
                number: Some(open.number),
                ..PushResult::of(Outcome::PrOpen, &target)
            },
            &head,
        );
    }
    if let Err(res) = push_save_branch(root, &https, &helper, &save, &target, &kept, say) {
        return record_push(root, *res, &head);
    }
    kept.pushed = Some(head.clone());
    kept.write(root);
    say(Say::Done(
        format!("Pushed {save} via {cli} (HTTPS token — no SSH, no 1Password).").into(),
    ));

    let (title, body) = describe(root, &target, &save, kind);
    let pr::Opened { pr: opened, ours } = match repo
        .backend()
        .open_or_update(caller, &repo.path, &save, &target, &title, &body)
    {
        Ok(opened) => opened,
        Err(why) => {
            say(Say::Warn(
                format!("Pushed {save}, but the {noun} into {target} could not be opened: {why}")
                    .into(),
            ));
            return record_push(
                root,
                PushResult {
                    landed: Some(save),
                    detail: why.to_string(),
                    ..PushResult::of(Outcome::Failed, &target)
                },
                &head,
            );
        }
    };
    let request = format!("{noun} {}", kind.request_ref(opened.number));
    // purlis merges a `pr-merge` save's request itself, and not now: this commit reached the
    // forge a moment ago, and no check on it has had time to start. The next look asks.
    let waits = (ours && mode == Mode::PrMerge)
        .then(|| Merging::new(MergeState::Waiting, pr::NOT_STARTED_YET));
    kept.pr = Some(KeptPr {
        number: opened.number,
        url: opened.url.clone(),
        head: head.clone(),
        target: target.clone(),
        merges_itself: waits.is_some(),
        pushed_at: Some(now_secs()),
        merge: waits.clone(),
    });
    kept.write(root);
    say(Say::Done(
        format!(
            "{} into {target}: {}",
            kind.request_noun_capitalised(),
            opened.url
        )
        .into(),
    ));
    let mut detail = String::new();
    if !ours {
        // A save branch somebody set by hand can carry a PR a person opened: it was left exactly
        // as it was, and nothing more is asked of the forge about it (#299's rule).
        detail = format!(
            "the open {noun} from the save branch is not purlis's, so it was left as it was"
        );
        say(Say::Info(format!("  {detail}.").into()));
    } else if let Some(waits) = waits {
        detail = waits.said(&request, &opened.url);
        say(Say::Info(format!("  {detail}.").into()));
    }
    record_push(
        root,
        PushResult {
            landed: Some(save),
            url: Some(opened.url),
            number: Some(opened.number),
            detail,
            ..PushResult::of(Outcome::PrOpen, &target)
        },
        &head,
    )
}

/// Merge the save's request ourselves, at `head`, once the forge's own rules and `gate` allow
/// it ([`crate::forge::Fallback::MergeWhenReady`], ADR 0051). Under [`Gate::Checks`], never
/// past a failing or pending check, a check that has not had time to start, or a review the
/// branch requires.
fn merge_it_ourselves(
    repo: &Repo,
    pr: &Pr,
    at: &Pinned,
    caller: &Caller,
    gate: Gate,
) -> Result<Ready, crate::forge::ForgeError> {
    repo.backend()
        .merge_when_ready(caller, &repo.path, pr, at, gate)
}

/// The origin URL and credential helper [`settle`] fetches the target branch with after purlis
/// merged the request itself: held to the remote recorded for the clone, as every other fetch
/// is ([`forge::gitcredential::helper_for_fetch`], #1864). `Ok(None)` when the origin is on no
/// forge purlis knows; `Err` when the clone's remote is not the recorded one, and nothing is
/// fetched.
fn fetch_after_merge(
    root: &Path,
    on: &forge::Forge,
) -> Result<Option<(String, String)>, &'static str> {
    let Some(https) = origin_https(root) else {
        return Ok(None);
    };
    let helper = forge::gitcredential::helper_for_fetch(on, root)?;
    Ok(Some((https, helper)))
}

/// Push HEAD to `save`, leased against the commit this clone last pushed there (`kept`).
///
/// - **Nothing kept** — the first push from this clone — leases the branch as absent. A
///   branch that is there anyway is pushed over only when HEAD already has every commit on
///   it ([`tip_is_in_head`]): in its history, or under another sha as a history rewritten
///   with the same changes leaves them. Nothing is lost; otherwise it is somebody else's, and
///   the plane is blocked.
/// - **A tip other than the one kept** is held to the same rule, and the push that follows
///   is leased against the tip that was seen, so a commit landing in between still blocks.
/// - **A branch that is gone** — deleted after its PR merged, even between a first push and
///   the listing after it — is leased again as absent.
fn push_save_branch(
    root: &Path,
    https: &str,
    helper: &str,
    save: &str,
    target: &str,
    kept: &Kept,
    say: Sink,
) -> Result<(), Box<PushResult>> {
    let failed = |detail: String, say: Sink| {
        say(Say::Warn(
            format!("Committed, but pushing {save} failed:").into(),
        ));
        for line in detail.lines() {
            say(Say::Warn(format!("  {line}").into()));
        }
        Box::new(PushResult {
            landed: Some(save.to_string()),
            detail,
            ..PushResult::of(Outcome::Failed, target)
        })
    };
    let push = |expected: &str| {
        git::push_head_by_id(
            root,
            Some(helper),
            &[&format!("--force-with-lease=refs/heads/{save}:{expected}")],
            https,
            &format!("refs/heads/{save}"),
        )
    };
    let expected = kept.pushed.clone().unwrap_or_default();
    let mut pushed = push(&expected).map_err(|unavailable| failed(unavailable.to_string(), say))?;
    // Only a stale lease sends charter back to the remote. A push that landed is done, and one
    // refused for a reason of the remote's own keeps that reason: a listing after it that
    // failed would put "could not read from remote" in its place.
    if !pushed.ok() && pushed.err.contains("stale info") {
        // What the remote has instead: nothing, or a tip.
        let listed = git::run_network(
            root,
            Some(helper),
            &["ls-remote", "--heads", https, &format!("refs/heads/{save}")],
        )
        .map_err(|unavailable| failed(unavailable.to_string(), say))?;
        if !listed.ok() {
            return Err(failed(tail(&listed), say));
        }
        let tip = listed
            .out
            .split_whitespace()
            .next()
            .filter(|sha| is_object_name(sha))
            .map(str::to_string);
        let again = match &tip {
            // Gone from the remote: nobody's commits are on it, so it is leased again as
            // absent — whatever was expected. With nothing expected the first push leased it as
            // absent too, and found it there: deleted between that push and this listing, as
            // the forge deletes a merged request's branch. A clone that never pushed this
            // branch has no request of its own to settle first, so pushing it afresh is what
            // its next save would do anyway, and blocking would only stop auto-save over a race
            // (D-HY8g, amended after review).
            None => Some(String::new()),
            // Every commit on it is already in HEAD — this clone's own earlier push, unrecorded,
            // or that push's commits under new shas after the history was rewritten — so
            // nothing of anybody's is lost. Leased against this tip, not the one kept. The tip
            // has to be here to be asked about.
            Some(tip) if tip_is_in_head(root, https, helper, save, tip) => Some(tip.clone()),
            // A commit on it that HEAD has under no sha: somebody else's, never overwritten.
            _ => None,
        };
        if let Some(again) = again {
            pushed = push(&again).map_err(|unavailable| failed(unavailable.to_string(), say))?;
        }
    }
    if pushed.ok() {
        return Ok(());
    }
    if pushed.err.contains("stale info") {
        let why = format!(
            "{save} on the remote has commits this clone did not push there — another clone, or \
             another machine with the same name — so purlis will not overwrite them. Name a \
             save_branch of this clone's own under [plane] in charter.local.toml, or delete \
             {save} on the remote if nothing on it is wanted, and save again"
        );
        say(Say::Fail(format!("Not pushed: {why}.").into()));
        return Err(Box::new(PushResult {
            landed: Some(save.to_string()),
            detail: why,
            ..PushResult::of(Outcome::Blocked, target)
        }));
    }
    Err(failed(tail(&pushed), say))
}

/// Whether HEAD already has every commit on the remote save branch's `tip`, fetching it first
/// if this clone does not have it: the tip is in HEAD's history, or each commit on it that
/// HEAD's history lacks is one HEAD has under another sha — the same change, as `git cherry`
/// reads it. On 2026-10-13 the plane's history was rewritten with the same trees and new
/// dates, which left every commit on the save branch under an old sha, and every save
/// blocked. A `git cherry` that did not run is a no: nothing is pushed over what could not be
/// read.
fn tip_is_in_head(root: &Path, https: &str, helper: &str, save: &str, tip: &str) -> bool {
    if resolve(root, &format!("{tip}^{{commit}}")).is_none() {
        let _ = git::run_network(
            root,
            Some(helper),
            &[
                "fetch",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                https,
                &format!("+refs/heads/{save}:refs/remotes/origin/{save}"),
            ],
        );
    }
    is_ancestor(root, tip, "HEAD") || nothing_but_equivalents(root, tip)
}

/// Whether every commit `tip` has that HEAD lacks is patch-equivalent to one HEAD has: `git
/// cherry HEAD <tip>` ran and marked none of them `+`.
fn nothing_but_equivalents(root: &Path, tip: &str) -> bool {
    git::run(root, &["cherry", "HEAD", tip], git::READ)
        .is_ok_and(|r| r.ok() && !r.out.lines().any(|line| line.starts_with('+')))
}

/// The request's title and body: what the save branch carries that the target lacks, in the
/// commits' own words, and the request named as `kind`'s forge names it.
fn describe(root: &Path, target: &str, save: &str, kind: forge::Kind) -> (String, String) {
    const LISTED: usize = 50;
    let range = format!("refs/remotes/origin/{target}..HEAD");
    let subjects: Vec<String> = git::run(
        root,
        &[
            "log",
            "-n",
            &LISTED.to_string(),
            "--reverse",
            "--format=%s",
            &range,
        ],
        git::READ,
    )
    .map(|r| r.out.lines().map(str::to_string).collect())
    .unwrap_or_default();
    let total = count(root, &range).map_or(subjects.len(), |n| n as usize);
    let host = crate::dispatch::host();
    // The forge keeps the request: the program as the plane spells it (D-RN11a-4).
    let program = crate::names::BINARY.writes_for(root);
    // `total == 1` is the subjects' own count whenever there is one subject: they are the newest
    // `LISTED` of the same range, and `total` falls back to their count. Excluded as
    // equivalent in `.cargo/mutants.toml`.
    let title = match subjects.as_slice() {
        [one] if total == 1 => one.clone(),
        _ => format!("{total} saves from {host}"),
    };
    let mut body = format!(
        "Saved by {program} on {host}. Every save from this machine pushes to {save} and updates \
         this {}.\n",
        kind.request_noun()
    );
    if total > subjects.len() {
        body.push_str(&format!("\n- … and {} earlier", total - subjects.len()));
    }
    for subject in &subjects {
        body.push_str(&format!("\n- {subject}"));
    }
    (title, body)
}

/// What [`settle`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Settled {
    /// No PR is known.
    Nothing,
    /// The PR is open.
    Open,
    /// The PR merged, and the plane was moved onto the remote's target branch.
    Moved,
    /// The PR merged, and the plane cannot be moved yet: a file is in the way, or newer
    /// commits wait on a clean tree. Not blocked: the next look tries again.
    Waiting(String),
    /// The forge could not say where the PR stands. A save stops before it pushes.
    Unknown(String),
    /// The PR was closed without merging, or merged as a commit that does not hold what was
    /// pushed. Recorded.
    Blocked(PushResult),
}

/// Settle the pull request this clone last opened, if it has merged or closed since.
///
/// `sign` is whether commits a rebase replays are signed, as the save's own commit is.
/// `fetch` is the origin URL and credential helper to fetch the target branch with, for a
/// caller that has not just fetched it; `None` when it has ([`super::fetch`]). `caller` is who
/// the forge is asked as.
pub(super) fn settle(
    root: &Path,
    plane: &Plane,
    target: &str,
    sign: bool,
    fetch: Option<(&str, &str)>,
    caller: &Caller,
    say: Sink,
) -> Settled {
    let mut kept = Kept::read(root);
    let Some(known) = kept.pr.clone() else {
        return Settled::Nothing;
    };
    if known.target != target {
        return Settled::Nothing;
    }
    // A plane moved off the commit the PR carries — by a person clearing a block — is no
    // longer the plane that PR is about.
    if !is_ancestor(root, &known.head, "HEAD") {
        kept.pr = None;
        // Not seen to end: a pre-rename branch is held until a settle sees it end.
        kept.held = kept.branch.as_deref().is_some_and(|b| {
            b != plane.save_branch_or_default(root) && is_own_default(root, plane, b)
        });
        kept.write(root);
        return Settled::Nothing;
    }
    let Ok((repo, save, _)) = config(root, plane) else {
        return Settled::Nothing;
    };
    let number = known.number;
    let kind = repo.forge.kind;
    // `pull request #12`, `merge request !12`.
    let request = format!("{} {}", kind.request_noun(), kind.request_ref(number));
    let head = known.head.clone();
    let opened = Pr {
        number,
        url: known.url.clone(),
    };
    let blocked = |detail: String, forget: bool, say: Sink| {
        say(Say::Fail(format!("Blocked: {detail}").into()));
        if forget {
            let mut kept = Kept::read(root);
            kept.pr = None;
            forget_old_branch(root, plane, &mut kept);
            kept.write(root);
        }
        Settled::Blocked(record_push(
            root,
            PushResult {
                landed: Some(save.clone()),
                url: Some(opened.url.clone()),
                number: Some(number),
                detail,
                ..PushResult::of(Outcome::Blocked, target)
            },
            &head,
        ))
    };
    let mut asked = repo.backend().state(caller, &repo.path, &opened);
    // A `pr-merge` save's request is merged here, by purlis, once it is ready: every save,
    // every fetch and every look of the app's asks again until then.
    let mut merged_here = false;
    // Only for a plane whose mode asks for the merge, whatever is kept beside the request:
    // that file is a hint, and the settings are what the project declared.
    let merges = known.merges_itself && plane.mode.value == Some(Mode::PrMerge);
    if matches!(asked, Ok(State::Open)) && merges {
        if stamp_is_unusable(known.pushed_at, now_secs()) {
            restamp(root);
        }
        // What is kept here can only make the look wait: the backend holds a settled head to
        // the forge's own clock, and asks the forge whose request this is.
        let gate = Gate::Checks {
            settled: settled(known.pushed_at),
        };
        // The one request this save may merge: at the commit it pushed, from this clone's own
        // save branch, into the plane's target. The forge's answer must match all three.
        let at = Pinned {
            sha: &head,
            branch: &save,
            base: target,
        };
        let looked = merge_it_ourselves(&repo, &opened, &at, caller, gate);
        let merging = Merging::of_a_look(&looked, &repo.forge, || {
            crate::forge::signin::ready(&repo.forge, caller)
        });
        let queued = matches!(looked, Ok(Ready::Queued(_)));
        // Kept where the window reads it, whatever comes next: the request's own state.
        if remember(root, &merging, queued) {
            // Said in the push record too, while that record is still this request's own: one
            // a later push that failed wrote is about a commit this request does not carry.
            let its_own = push_record(root).is_some_and(|rec| {
                rec.get("outcome").and_then(serde_json::Value::as_str) == Some("pr-open")
            });
            if its_own {
                note_open(
                    root,
                    target,
                    &save,
                    &known,
                    &merging.said(&request, &known.url),
                );
            }
            if matches!(merging.state, MergeState::Red | MergeState::CannotMerge) {
                say(Say::Warn(merging.said(&request, &known.url).into()));
            }
        }
        kept = Kept::read(root);
        if !matches!(looked, Ok(Ready::Merged)) {
            return Settled::Open;
        }
        merged_here = true;
        say(Say::Done(
            format!("purlis merged {request} once it was ready.").into(),
        ));
        asked = repo.backend().state(caller, &repo.path, &opened);
    }
    // What purlis merged just now is not in the fetch its caller made before asking. That fetch
    // is held to the recorded remote, as the caller's was (#1864).
    let own_fetch = if merged_here && fetch.is_none() {
        match fetch_after_merge(root, &repo.forge) {
            Ok(own) => own,
            Err(why) => {
                return Settled::Unknown(format!(
                    "{request} merged, but {target} was not fetched to move onto it: {why}"
                ));
            }
        }
    } else {
        None
    };
    let fetch = fetch.or(own_fetch
        .as_ref()
        .map(|(https, helper)| (https.as_str(), helper.as_str())));
    let commit = match asked {
        Ok(State::Open) => return Settled::Open,
        Ok(State::Closed) => {
            return blocked(
                format!(
                    "{request} was closed without merging. This machine's commits are still \
                     here: save again to open a new {}",
                    kind.request_noun()
                ),
                true,
                say,
            );
        }
        Ok(State::Merged { commit }) => commit,
        Err(why) => {
            return Settled::Unknown(format!(
                "purlis could not ask where {request} stands: {why}"
            ));
        }
    };
    if let Some((https, helper)) = fetch {
        let fetched = git::run_network(
            root,
            Some(helper),
            &[
                "fetch",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                https,
                &format!("+refs/heads/{target}:refs/remotes/origin/{target}"),
            ],
        );
        if !fetched.is_ok_and(|run| run.ok()) {
            return Settled::Unknown(format!(
                "{request} merged, but {target} could not be fetched to move onto it"
            ));
        }
    }
    let theirs = format!("refs/remotes/origin/{target}");
    if !is_ancestor(root, &head, &theirs) {
        // The commit the merge made, which must be on the target; the target's tip only when
        // the forge named none.
        let merged_at = match commit.filter(|c| is_object_name(c)) {
            Some(commit) => {
                if !(resolve(root, &format!("{commit}^{{commit}}")).is_some()
                    && is_ancestor(root, &commit, &theirs))
                {
                    return blocked(
                        format!(
                            "{request} merged as {commit}, which is not on \
                             {target} on the remote. Nothing was moved"
                        ),
                        false,
                        say,
                    );
                }
                commit
            }
            None => theirs.clone(),
        };
        let differ = differing(root, &head, &merged_at);
        if !differ.is_empty() {
            let shown: Vec<&str> = differ.iter().take(5).map(String::as_str).collect();
            let more = differ.len().saturating_sub(shown.len());
            return blocked(
                format!(
                    "{request} merged, but what it merged does not match what this \
                     machine pushed in {}{}. Nothing was moved",
                    shown.join(", "),
                    if more > 0 {
                        format!(" and {more} more")
                    } else {
                        String::new()
                    }
                ),
                false,
                say,
            );
        }
    }
    // Under a chat's sandbox the move would stop at a file it may not write, part-way (qw346):
    // the plane waits, and the window's own look, under no sandbox, makes it.
    let stops = super::sandbox_would_stop(root, &theirs);
    if !stops.is_empty() {
        return Settled::Waiting(format!(
            "{request} merged, and {target} moves onto the remote's from the window, not from \
             this chat: {}",
            super::said_sandbox_stops(&stops)
        ));
    }
    let newer = count(root, &format!("{head}..HEAD")).unwrap_or(0);
    if newer == 0 {
        // `--keep` keeps uncommitted work, and refuses rather than overwrite a file with some:
        // then the plane waits, and the next look tries again.
        match git::run(root, &["reset", "-q", "--keep", &theirs], WRITE) {
            Ok(run) if run.ok() => {}
            Ok(run) => {
                return Settled::Waiting(format!(
                    "{request} merged, and {target} moves onto the remote's once \
                     nothing here is in the way: {}",
                    tail(&run)
                ));
            }
            Err(unavailable) => {
                return Settled::Waiting(format!(
                    "{request} merged, and moving {target} did not finish: \
                     {unavailable}"
                ));
            }
        }
    } else {
        if !changed_paths(root).is_empty() {
            // Newer commits are replayed onto the remote's branch, which needs a clean tree:
            // the next save commits first, and settles then.
            return Settled::Waiting(format!(
                "{request} merged, and {target} moves onto the remote's with the \
                 next save"
            ));
        }
        let run = git::run(
            root,
            &[
                "-c",
                gpgsign(sign),
                "rebase",
                "-q",
                "--onto",
                &theirs,
                &head,
            ],
            WRITE,
        );
        if !run.as_ref().is_ok_and(git::Run::ok) {
            let _ = git::run(root, &["rebase", "--abort"], WRITE);
            let said = match &run {
                Ok(run) => tail(run),
                Err(unavailable) => unavailable.to_string(),
            };
            return blocked(
                format!(
                    "{request} merged, but the {} made since would \
                     not replay onto {target}: {said}",
                    counted(newer, "commit", "commits")
                ),
                false,
                say,
            );
        }
    }
    kept.pr = None;
    forget_old_branch(root, plane, &mut kept);
    kept.write(root);
    let _ = std::fs::remove_file(push_record_path(root));
    say(Say::Done(
        format!(
            "{} {} merged — {target} is now the remote's{}.",
            kind.request_noun_capitalised(),
            kind.request_ref(number),
            if newer > 0 {
                format!(
                    ", with {} on top",
                    counted(newer, "newer commit", "newer commits")
                )
            } else {
                String::new()
            }
        )
        .into(),
    ));
    Settled::Moved
}

/// Rewrite the push record of the request `known` still open from `save`, with `detail`: what a
/// later look found, said where the window reads it. The record's own head is kept, so the
/// standing it gives is the one it gave.
fn note_open(root: &Path, target: &str, save: &str, known: &KeptPr, detail: &str) {
    let head = read_json(&push_record_path(root))
        .and_then(|rec| {
            rec.get("head")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|h| is_object_name(h))
        .unwrap_or_else(|| known.head.clone());
    record_push(
        root,
        PushResult {
            landed: Some(save.to_string()),
            url: Some(known.url.clone()),
            number: Some(known.number),
            detail: detail.to_string(),
            ..PushResult::of(Outcome::PrOpen, target)
        },
        &head,
    );
}

/// The paths the PR touched — what `head` changed since it forked from `theirs` — whose
/// content in `theirs` is not what `head` has.
fn differing(root: &Path, head: &str, theirs: &str) -> Vec<String> {
    let Some(base) = git::run(root, &["merge-base", head, theirs], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
    else {
        return vec!["(no common history)".to_string()];
    };
    let names = |args: &[&str]| -> Option<Vec<String>> {
        let mut argv = vec![
            "--literal-pathspecs",
            "diff",
            "--no-renames",
            "--name-only",
            "-z",
        ];
        argv.extend_from_slice(args);
        git::run(root, &argv, git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|r| {
                r.out
                    .split('\0')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect()
            })
    };
    let Some(touched) = names(&[&base, head]) else {
        return vec!["(git could not compare them)".to_string()];
    };
    if touched.is_empty() {
        return Vec::new();
    }
    let mut args: Vec<&str> = vec![head, theirs, "--"];
    args.extend(touched.iter().map(String::as_str));
    names(&args).unwrap_or_else(|| vec!["(git could not compare them)".to_string()])
}

/// What the person's own "merge it as it stands" came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergedAsItStands {
    /// Merged, and the project moved onto what merged.
    Moved,
    /// Merged or queued on the forge; the project moves onto it with the next look. Carries
    /// what happened, in words.
    Later(String),
}

/// **Merge the save's request as it stands**, failed and missing checks included: the person's
/// own, explicit action in the window (the operator's ruling P1, 2026-10-13), and nothing a
/// save, a look or a chat ever does. Only the checks are passed over ([`Gate::Person`]): a
/// conflict, a review the branch requires and every rule the forge enforces still refuse it,
/// and it merges only what `seen` says the person was shown (#1978): at that commit, which
/// must still be the one purlis pushed, and in that standing, both read again under the claim.
pub(super) fn merge_as_it_stands(
    root: &Path,
    plane: &Plane,
    target: &str,
    sign: bool,
    seen: &super::Seen,
    caller: &Caller,
    say: Sink,
) -> Result<MergedAsItStands, String> {
    if plane.mode.value != Some(Mode::PrMerge) {
        return Err(
            "this project's saves do not merge their pull request ([plane] mode is not \
             pr-merge), so there is nothing for purlis to merge"
                .into(),
        );
    }
    let kept = Kept::read(root);
    let Some(known) = kept.pr.filter(|pr| pr.target == target) else {
        return Err("no pull request of this project's saves is open".into());
    };
    if let Some(changed) = super::changed_since(seen, &known.head, known.merge.as_ref()) {
        return Err(changed);
    }
    if !is_ancestor(root, &known.head, "HEAD") {
        return Err(
            "this project is no longer on the commit its open pull request carries, so purlis \
             will not merge it. Save again first"
                .into(),
        );
    }
    let (repo, save, _) = config(root, plane)?;
    let kind = repo.forge.kind;
    let request = format!("{} {}", kind.request_noun(), kind.request_ref(known.number));
    let opened = Pr {
        number: known.number,
        url: known.url.clone(),
    };
    let at = Pinned {
        sha: &seen.head,
        branch: &save,
        base: target,
    };
    let looked = merge_it_ourselves(&repo, &opened, &at, caller, Gate::Person);
    match &looked {
        Ok(Ready::Merged) | Ok(Ready::Queued(_)) => {}
        Ok(Ready::Waiting(why)) | Ok(Ready::Stalled(why)) | Ok(Ready::Refused(why)) => {
            return Err(format!("{request} was not merged: {why}"));
        }
        Ok(Ready::Red(_)) => return Err(format!("{request} was not merged")),
        Err(why) => return Err(format!("{request} was not merged: {why}")),
    }
    let merging = Merging::of(&looked);
    remember(root, &merging, matches!(looked, Ok(Ready::Queued(_))));
    if let Ok(Ready::Queued(why)) = &looked {
        say(Say::Done(why.clone().into()));
        return Ok(MergedAsItStands::Later(why.clone()));
    }
    say(Say::Done(
        format!("Merged {request} as it stands, as you asked: {}", known.url).into(),
    ));
    let fetch = fetch_after_merge(root, &repo.forge).map_err(str::to_string)?;
    let fetch = fetch
        .as_ref()
        .map(|(https, helper)| (https.as_str(), helper.as_str()));
    match settle(root, plane, target, sign, fetch, caller, say) {
        Settled::Moved => Ok(MergedAsItStands::Moved),
        Settled::Blocked(res) => Err(res.detail),
        Settled::Waiting(why) | Settled::Unknown(why) => Ok(MergedAsItStands::Later(why)),
        Settled::Nothing | Settled::Open => Ok(MergedAsItStands::Later(format!(
            "{request} merged, and {target} moves onto it with the next look"
        ))),
    }
}

/// **What the window's Fix hands a chat about the project's red request**: its number, and the
/// request with its failed checks and what each found, asked of the forge now
/// ([`crate::savefix::read_failed`]). Refused unless the last look found it red.
pub(super) fn trouble(
    root: &Path,
    plane: &Plane,
    target: &str,
    caller: &Caller,
) -> Result<(u64, crate::savefix::Trouble), String> {
    let kept = Kept::read(root);
    let Some(known) = kept.pr.filter(|pr| pr.target == target) else {
        return Err("no pull request of this project's saves is open".into());
    };
    let red = known
        .merge
        .as_ref()
        .filter(|merge| merge.state == super::MergeState::Red);
    let Some(red) = red else {
        return Err(
            "the pull request of this project's saves is not red any more, so there is \
             nothing to fix"
                .into(),
        );
    };
    let (repo, _, _) = config(root, plane)?;
    let kind = repo.forge.kind;
    let request = format!("{} {}", kind.request_noun(), kind.request_ref(known.number));
    let failed = crate::savefix::read_failed(
        repo.backend().as_ref(),
        caller,
        &repo.path,
        &known.head,
        known.number,
        &red.checks,
    )
    .map_err(|not| not.said(&request))?;
    Ok((
        known.number,
        crate::savefix::red_request(
            &request,
            &known.url,
            &repo,
            crate::savefix::Whose::Project,
            failed,
        ),
    ))
}

/// The origin URL and credential helper a look that may find the request merged fetches the
/// target branch with ([`fetch_after_merge`]), for the plane's own forge. `Ok(None)` where
/// there is nothing to fetch from.
pub(super) fn fetch_for_a_look(
    root: &Path,
    plane: &Plane,
) -> Result<Option<(String, String)>, String> {
    let Ok((repo, _, _)) = config(root, plane) else {
        return Ok(None);
    };
    fetch_after_merge(root, &repo.forge).map_err(str::to_string)
}

#[cfg(test)]
mod tests;
