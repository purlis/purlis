// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Saving a workspace repo by its `[repos.<name>]` mode (charter-app#299, ADR 0051).
//!
//! The plane's save ([`crate::planegit::save_as`]) is the model, and this reuses its parts
//! where they apply to a clone: the hardened git runner, the claim that keeps two saves of one
//! tree apart, the signing `-c`, the forge CLI's credential helper for the push, and the save
//! journal, where a repo's line carries the target `repo:<workspace>/<name>`.
//!
//! # What a repo save does
//!
//! It commits on the branch the clone is on — `git add -A`, with the message it was given or
//! one listing the files — and then goes as far as the mode:
//!
//! - `off`: nothing at all.
//! - `commit`: it stops at the commit.
//! - `push`: it pushes that branch.
//! - `pr` and `pr-merge`: it pushes that branch and opens or updates a pull request from it
//!   into `branch`, which defaults to the repo's `default_branch` in `inventory/repos.json`
//!   (else the clone's `origin/HEAD`). In `pr-merge` purlis then merges it itself, pinned to
//!   the pushed commit, once every check there has passed — never at the save, whose commit
//!   has no check yet, and never past a failed check, required or not ([`look`]; ADR 0051,
//!   amended 2026-10-13). The forge's own auto-merge is never asked for.
//!
//! **A request mode never pushes to the default branch or to the PR's base.** A clone standing on
//! either gets a branch of its own first, `purlis/<workspace>/<short-sha>`, created at HEAD
//! and pushed instead. The clone stays where it is: a save commits on the branch the repo is
//! on, and nothing moves an agent's checkout under it. The next save from there reuses that
//! branch while it is an ancestor of HEAD, so one pull request collects the saves — a
//! `charter/<workspace>/…` branch made before the rename included, so its open pull request
//! carries on rather than being opened a second time (V93j).
//!
//! # What it does not do
//!
//! - **No rebase.** The plane's push rebases onto a remote that moved; a repo's push does not.
//!   A clone's history is its developers', and nothing rewrites anyone's history (ADR 0051): a
//!   push the remote refuses leaves the repo blocked, in git's words.
//! - **Not the plane's secret rule.** The plane's guard reads memory and ref files with a
//!   credential-assignment rule that refuses ordinary code (`password: String`). A repo save
//!   refuses by a narrower rule instead, [`secret_files`]: a file named like a credential, or a
//!   private key or a forge token in anything staged (ADR 0051).
//! - **Nobody else's pull request.** An open PR from the same branch that charter did not open
//!   — its head is not under `purlis/` or `charter/` and its body has no [`pr::MARKER`], in
//!   either spelling — keeps its title,
//!   its description and its merging; the save names it and leaves it.
//!
//! # A session mid-turn
//!
//! A repo is where a workspace's chats work, so a save waits while any of them is mid-turn
//! (ADR 0051). The caller says which are, by hook state ([`Request::mid_turn`]). A save the
//! operator asked for is refused with a sentence naming them — it does not queue itself, so
//! nothing runs later that nobody is watching — and an auto-save skips that cycle.

use std::path::Path;

use crate::counts::{counted, plural};
use crate::forge::{self, Forge, pr};
use crate::gitstate;
use crate::planegit::{self, Claim, Looked, MergeState, MergedAsItStands, Merging, Stage, Trigger};
use crate::planesave::{self, Mode};
use crate::repocmd::{Say, Sink};
use crate::repos::{self, Head};
use crate::shown;
use crate::worktree::git;

/// What a repo save was asked for.
pub struct Request<'a> {
    /// The plane: its `charter.toml` says the mode, and its journal records the save.
    pub plane: &'a Path,
    pub workspace: &'a str,
    /// The repo's name — its `[repos.<name>]` table and its clone's directory.
    pub name: &'a str,
    /// The clone, as [`repos::clones`] checked it.
    pub clone: &'a Path,
    /// The commit message, or `None` for one listing the files.
    pub message: Option<&'a str>,
    /// Commit only, whatever the mode: the commit a quit makes before its bounded push.
    pub no_push: bool,
    /// The chats that are mid-turn where they could be writing this clone, by the names the
    /// window shows. Any at all, and the save waits. **Asked, not handed over**: it is asked
    /// once before the save starts and again right before `git add -A`, because a turn can
    /// start while a save waits for its claim or its first git call.
    pub mid_turn: &'a dyn Fn() -> Vec<String>,
}

/// A [`Request::mid_turn`] that says no chat is working: nothing in this process can be.
///
/// cargo-mutants' `vec![]` for it is this very body, so `.cargo/mutants.toml` excludes that one
/// mutant as equivalent.
pub fn nobody_working() -> Vec<String> {
    Vec::new()
}

/// The journal's `target` for a repo.
pub fn target(workspace: &str, name: &str) -> String {
    format!("repo:{workspace}/{name}")
}

/// The sentence a save held back by working sessions says.
pub fn waiting_for(workspace: &str, mid_turn: &[String]) -> String {
    let who = match mid_turn {
        [one] => format!("{one} is"),
        [first @ .., last] => format!("{} and {last} are", first.join(", ")),
        [] => "a session is".to_owned(),
    };
    format!(
        "Not saved: {who} mid-turn in {workspace}. A repo is saved between turns — save again \
         when the turn ends."
    )
}

/// The sentence a save refused for its clone's [`Claim`] says.
pub fn already_saving(workspace: &str, name: &str) -> String {
    format!("A save of {workspace}/{name} is already running — wait for it to finish.")
}

/// Save a repo, as `trigger` asked. Returns the exit status; every save that ran leaves one
/// line in the plane's save journal.
///
/// A save held back by a session mid-turn before it starts journals nothing: nothing was
/// attempted. The window's button hears why (exit 1); an auto-save hears nothing and tries next
/// cycle. A save at quit is journalled as skipped, since there is no next cycle to try in.
pub fn save_as(request: &Request, trigger: Trigger, say: Sink) -> u8 {
    let busy = (request.mid_turn)();
    if !busy.is_empty() && trigger != Trigger::Quit {
        if matches!(trigger, Trigger::Manual | Trigger::Cli) {
            say(Say::Fail(waiting_for(request.workspace, &busy).into()));
            return 1;
        }
        return 0;
    }
    let Some(claim) = Claim::of(request.clone) else {
        say(Say::Fail(
            already_saving(request.workspace, request.name).into(),
        ));
        return 1;
    };
    save_claimed(request, trigger, &claim, say)
}

/// [`save_as`], for a caller that already holds the clone's [`Claim`].
pub fn save_claimed(request: &Request, trigger: Trigger, _claim: &Claim, say: Sink) -> u8 {
    let started = std::time::Instant::now();
    let repo = planesave::Settings::read(request.plane).repo(request.name);
    let mode = if request.no_push && repo.mode.value != Mode::Off {
        Mode::Commit
    } else {
        repo.mode.value
    };
    let mut attempt = Attempt::default();
    let mut refused: Option<String> = None;
    let mut heard = |line: Say| {
        if let Say::Fail(text) = &line
            && refused.is_none()
        {
            refused = Some(text.clone().to_string());
        }
        say(line);
    };
    let say: Sink = &mut heard;
    let code = if mode == Mode::Off {
        say(Say::Info(
            format!(
                "[repos.{}] mode is off ({}), so purlis commits nothing in {}/{}.",
                request.name,
                repo.mode.source.file().unwrap_or("default"),
                request.workspace,
                request.name
            )
            .into(),
        ));
        attempt.outcome = "skipped";
        0
    } else {
        commit_push(request, trigger, &repo, mode, &mut attempt, say)
    };
    if attempt.detail.is_empty()
        && let Some(refused) = refused
    {
        attempt.detail = refused;
    }
    planegit::journal_append(
        request.plane,
        &serde_json::json!({
            "at": planegit::now(),
            "target": target(request.workspace, request.name),
            "trigger": trigger.word(),
            "mode": mode.as_str(),
            "files": attempt.files,
            "commit": attempt.commit,
            "branch": attempt.branch,
            "pr": attempt.pr,
            "number": attempt.number,
            "merge": attempt.merge.as_ref().map(Merging::to_json),
            "pushed_at": attempt.merge.as_ref().map(|_| planegit::now() as u64),
            "ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "outcome": attempt.outcome,
            "detail": attempt.detail,
            "stopped": attempt.stopped,
        }),
    );
    code
}

/// What one repo save came to — the journal's line.
#[derive(Debug)]
struct Attempt {
    outcome: &'static str,
    files: usize,
    /// HEAD once the save was done with it: the commit it made or pushed.
    commit: Option<String>,
    /// The remote branch it pushed.
    branch: Option<String>,
    pr: Option<String>,
    /// The pull request's number, when a save opened or updated one of purlis's own.
    number: Option<u64>,
    /// Where a `pr-merge` save's request stands with merging.
    merge: Option<Merging>,
    detail: String,
    /// Blocked because git stopped part-way through something (#433). [`standing`] reads
    /// that from the clone as it is, so this line must not keep the repo blocked once the
    /// operator has finished or aborted it by hand.
    stopped: bool,
}

impl Default for Attempt {
    fn default() -> Self {
        Self {
            outcome: "failed",
            files: 0,
            commit: None,
            branch: None,
            pr: None,
            number: None,
            merge: None,
            detail: String::new(),
            stopped: false,
        }
    }
}

/// `git rev-parse <what>`, or `None`.
fn rev(clone: &Path, what: &str) -> Option<String> {
    git::run(clone, &["rev-parse", what], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|sha| !sha.is_empty())
}

fn commit_push(
    request: &Request,
    trigger: Trigger,
    repo: &planesave::Repo,
    mode: Mode,
    attempt: &mut Attempt,
    say: Sink,
) -> u8 {
    let clone = request.clone;
    let at = format!("{}/{}", request.workspace, request.name);
    let state = match repos::state_of(clone) {
        Ok(state) => state,
        Err(unreadable) => {
            say(Say::Fail(format!("Not saved: {unreadable}.").into()));
            attempt.outcome = "blocked";
            return 1;
        }
    };
    // Before the branch is asked for, since a rebase stopped part-way is on no branch, and
    // before anything is staged: `git add -A` would stage the conflict markers (#433).
    if let Some(stopped) = gitstate::stopped(clone) {
        let why = format!("in {at}, {}", stopped.why());
        say(Say::Fail(format!("Not saved: {why}.").into()));
        say(Say::Info(
            "  Nothing was staged or committed. Settle that, then save again.".into(),
        ));
        attempt.outcome = "blocked";
        attempt.detail = why;
        attempt.commit = rev(clone, "HEAD");
        attempt.stopped = true;
        return 1;
    }
    let branch = match &state.head {
        Head::Branch(branch) | Head::Unborn(branch) => branch.clone(),
        Head::Detached(sha) => {
            attempt.commit = rev(clone, "HEAD");
            let why = format!(
                "{at} is not on a branch (detached at {sha}), so a save has no branch to commit on"
            );
            say(Say::Fail(
                format!("Not saved: {why}. Check out a branch, then save again.").into(),
            ));
            attempt.outcome = "blocked";
            attempt.detail = why;
            return 1;
        }
    };

    // Asked again at the last moment before anything is staged: a turn that started while
    // this save waited must not have its half-written files committed.
    let busy = (request.mid_turn)();
    if !busy.is_empty() {
        let why = waiting_for(request.workspace, &busy);
        attempt.outcome = "skipped";
        attempt.detail = why.clone();
        if matches!(trigger, Trigger::Manual | Trigger::Cli) {
            say(Say::Fail(why.into()));
            return 1;
        }
        return 0;
    }

    // As the plane's save: a file the staging would take that has a second name refuses the
    // save before anything is staged, and each file is then staged from one open descriptor,
    // so a file with a second name is never hashed into the clone's object store
    // (`planegit::ondisk`). Staging's failure decides, never the probe after it.
    let refusal =
        match planegit::ondisk::refused_ahead(&planegit::ondisk::second_names_ahead(clone, &[])) {
            Some(refusal) => Some(refusal),
            None => {
                #[cfg(test)]
                tests::before_the_add(clone);
                let held = match planegit::ondisk::stage_named(clone, &[], |_| false) {
                    Ok(held) => held,
                    Err(said) => {
                        say(Say::Fail(
                            format!("Not saved: git could not stage the changes in {at} — {said}")
                                .into(),
                        ));
                        return 1;
                    }
                };
                #[cfg(test)]
                tests::after_the_add(clone);
                // A file whose own descriptor showed a second name was not staged.
                planegit::ondisk::refused(clone, &Ok(held))
            }
        };
    if let Some(refusal) = refusal {
        say(Say::Fail(
            format!("Not saved: a file in {at} is not the one file on disk:").into(),
        ));
        for line in refusal.named {
            say(Say::Fail(format!("  {line}").into()));
        }
        say(Say::Info(format!("  {}", refusal.then).into()));
        attempt.outcome = "blocked";
        attempt.detail = planegit::ondisk::NOT_AS_STAGED.into();
        attempt.commit = rev(clone, "HEAD");
        return 1;
    }
    let probe = git::run(clone, &["diff", "--cached", "--quiet"], git::READ)
        .ok()
        .and_then(|run| run.code);
    let committed = match probe {
        Some(0) => false,
        Some(1) => {
            let staged: Vec<String> =
                git::run(clone, &["diff", "--cached", "--name-only", "-z"], git::READ)
                    .map(|r| {
                        r.out
                            .split('\0')
                            .filter(|l| !l.trim().is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
            attempt.files = staged.len();
            // Each staged file is the one name it has on disk, as the plane's save asks: a row
            // with a second name, changed since it was staged, or gone is taken out of the
            // index, and its blob out of the clone's object store where nothing else reaches
            // it, before the secret check reads any blob. A row git does not look at is read by
            // that check and taken out after it. What the refusal says comes after
            // the check's own findings, as the plane's save says it (`planegit::ondisk`).
            let (first, after) =
                planegit::ondisk::split_for_guard(planegit::ondisk::not_as_staged(clone));
            let held_back: std::collections::HashSet<String> = first
                .as_ref()
                .map(|held| held.iter().map(|one| one.path.clone()).collect())
                .unwrap_or_default();
            let refused_first = planegit::ondisk::refused(clone, &first);
            #[cfg(test)]
            tests::at_the_secret_check(clone);
            // What a save must never carry to a remote: a secret-shaped file, or a private key
            // or a live token in anything it stages (ADR 0051, for a repo). Asked of the index,
            // as the plane's guard asks it, and refused before the commit exists; what was
            // staged stays staged, as the plane's refusal leaves it.
            // A row held back above is no longer staged: nothing of it is committed to scan.
            let to_scan: Vec<String> = staged
                .iter()
                .filter(|path| !held_back.contains(*path))
                .cloned()
                .collect();
            let flagged = secret_files(clone, &to_scan);
            let refusal = planegit::ondisk::Refusal::joined(
                refused_first,
                planegit::ondisk::refused(clone, &Ok(after)),
            );
            if !flagged.is_empty() {
                let named: Vec<String> = flagged
                    .iter()
                    .map(|(path, kind)| {
                        format!("{} ({kind})", shown::readable(path, shown::DISPLAY_LIMIT))
                    })
                    .collect();
                let why = format!(
                    "a secret-shaped file would be committed in {at}: {}",
                    named.join(", ")
                );
                say(Say::Fail(format!("Not saved: {why}.").into()));
                say(Say::Info(
                    "  Take it out of the tree or add it to .gitignore, then save again. Secrets \
                     belong in a vault."
                        .into(),
                ));
                if let Some(refusal) = refusal {
                    for line in refusal.named {
                        say(Say::Fail(format!("  {line}").into()));
                    }
                    say(Say::Info(format!("  {}", refusal.then).into()));
                }
                attempt.outcome = "blocked";
                attempt.detail = why;
                attempt.commit = rev(clone, "HEAD");
                return 1;
            }
            if let Some(refusal) = refusal {
                say(Say::Fail(
                    format!("Not saved: a staged file in {at} is not the one file on disk:").into(),
                ));
                for line in refusal.named {
                    say(Say::Fail(format!("  {line}").into()));
                }
                say(Say::Info(format!("  {}", refusal.then).into()));
                attempt.outcome = "blocked";
                attempt.detail = planegit::ondisk::NOT_AS_STAGED.into();
                attempt.commit = rev(clone, "HEAD");
                return 1;
            }
            let msg = match request.message {
                Some(text) if !text.trim().is_empty() => text.trim().to_string(),
                _ => summary(&staged, crate::names::BINARY.writes_for(request.plane)),
            };
            let sign = repo.sign.value;
            let mut commit: Vec<&str> = vec!["-c", planegit::gpgsign(sign)];
            commit.extend(["commit", "-q", "-m", msg.as_str()]);
            let made = git::run(clone, &commit, planegit::WRITE);
            let still_staged =
                git::run(clone, &["diff", "--cached", "--quiet"], git::READ).is_ok_and(|r| !r.ok());
            if still_staged {
                let why = match (sign, made) {
                    (true, Ok(run)) => format!(
                        "purlis could not sign the commit: {}",
                        planegit::signer_said(&run.err)
                    ),
                    (_, Ok(run)) => format!("git commit failed: {}", planegit::tail(&run)),
                    (_, Err(e)) => format!("git commit failed: {e}"),
                };
                say(Say::Fail(
                    format!(
                        "Not saved: {why} — {} in {at} {} staged but not committed.",
                        counted(staged.len(), "file", "files"),
                        plural(staged.len(), "is", "are")
                    )
                    .into(),
                ));
                attempt.detail = why;
                return 1;
            }
            let short = git::run(clone, &["rev-parse", "--short", "HEAD"], git::READ)
                .map(|r| r.line().trim().to_string())
                .unwrap_or_default();
            say(Say::Done(
                format!(
                    "Committed {short} on {branch} in {at}: {msg}  ({})",
                    counted(staged.len(), "file", "files")
                )
                .into(),
            ));
            true
        }
        _ => {
            say(Say::Fail(
                format!("Not saved: purlis could not tell whether {at} has anything staged.")
                    .into(),
            ));
            return 1;
        }
    };
    attempt.commit = rev(clone, "HEAD");
    attempt.outcome = if committed { "committed" } else { "skipped" };

    if mode == Mode::Commit {
        if !committed {
            say(Say::Info(format!("Nothing to save in {at}.").into()));
        } else if !request.no_push {
            say(Say::Info(
                format!(
                    "Not pushed: [repos.{}] mode is commit ({}).",
                    request.name,
                    repo.mode.source.file().unwrap_or("default")
                )
                .into(),
            ));
        }
        return 0;
    }
    let Some(head_sha) = attempt.commit.clone() else {
        say(Say::Info(
            format!("Nothing to save in {at}: {branch} has no commit yet.").into(),
        ));
        return 0;
    };
    let Some(https) = planegit::origin_https_of(request.plane, clone) else {
        let why = format!(
            "{at}'s origin is not on a GitHub or GitLab host this plane knows, so there is \
             nowhere to push"
        );
        say(Say::Warn(format!("{why}. Committed locally.").into()));
        if mode.opens_a_pr() {
            attempt.outcome = "blocked";
            attempt.detail = why;
            return 1;
        }
        return 0;
    };
    let forge = crate::gitpolicy::forge_for(clone, request.plane)
        .unwrap_or_else(|| Forge::default_of(forge::DEFAULT_KIND));
    // The window's own sign-in goes only to the remote recorded for this clone (#752).
    let helper = match forge::gitcredential::helper_for_push(&forge, clone, &https) {
        Ok(helper) => helper,
        Err(why) => {
            say(Say::Warn(format!("{at}: {why}. Committed locally.").into()));
            attempt.outcome = "blocked";
            attempt.detail = why.to_string();
            return 1;
        }
    };
    let noun = forge.kind.request_noun();

    // Where the commit goes: the branch it is on — or, in a request mode on the branch a PR would
    // go into, a branch of charter's own.
    let (base, remote_branch) = if mode.opens_a_pr() {
        let default = default_branch(request.plane, request.name, clone);
        let Some(base) = repo.branch.value.clone().or_else(|| default.clone()) else {
            let why = format!(
                "purlis does not know {}'s default branch, so it cannot say where a {noun} \
                 goes. Set [repos.{}] branch",
                request.name, request.name
            );
            say(Say::Fail(format!("Not pushed: {why}.").into()));
            attempt.outcome = "blocked";
            attempt.detail = why;
            return 1;
        };
        let protected = branch == base || default.as_deref() == Some(branch.as_str());
        let remote = if protected {
            match save_branch(request, &branch, &head_sha) {
                Ok(name) => name,
                Err(why) => {
                    say(Say::Fail(format!("Not pushed: {why}.").into()));
                    attempt.outcome = "blocked";
                    attempt.detail = why;
                    return 1;
                }
            }
        } else {
            branch.clone()
        };
        (Some(base), remote)
    } else {
        (None, branch.clone())
    };

    let pushed = git::run_network(
        clone,
        Some(helper.as_str()),
        &["push", &https, &format!("HEAD:refs/heads/{remote_branch}")],
    );
    let run = match pushed {
        Ok(run) => run,
        Err(unavailable) => {
            say(Say::Warn(
                format!("Committed, but the push failed: {unavailable}").into(),
            ));
            attempt.detail = unavailable.to_string();
            attempt.outcome = "failed";
            return 1;
        }
    };
    if !run.ok() {
        // All of what git said is asked, and what is repeated leaves its hints out: the
        // rejection's own line comes before four lines of advice.
        let all = if run.err.is_empty() {
            &run.out
        } else {
            &run.err
        };
        let said = all
            .lines()
            .filter(|line| !line.trim_start().starts_with("hint:") && !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let why = if planegit::is_protected_rejection(all) {
            let fix = if mode.opens_a_pr() {
                // Already a request mode, on a branch that is neither the default nor the base: the
                // branch itself is protected, so saves go through a PR into it instead.
                format!(
                    "Set [repos.{}] branch = \"{remote_branch}\" to save it through a {noun} \
                     into it, or work on another branch",
                    request.name
                )
            } else {
                format!(
                    "Set [repos.{}] mode = \"pr\" to save it through a {noun}",
                    request.name
                )
            };
            format!("{remote_branch} is protected and takes no direct push. {fix}")
        } else if ["fetch first", "non-fast-forward"]
            .iter()
            .any(|s| all.contains(s))
        {
            format!(
                "the remote's {remote_branch} has commits {at} does not. Bring them in, then \
                 save again"
            )
        } else {
            attempt.outcome = "failed";
            said.clone()
        };
        if attempt.outcome != "failed" {
            attempt.outcome = "blocked";
        }
        say(Say::Fail(
            format!("Committed, but not pushed: {why}").into(),
        ));
        for line in said.lines() {
            say(Say::Info(format!("  {line}").into()));
        }
        attempt.detail = why;
        return 1;
    }
    // Kept in step, so the next look counts against what the remote now has.
    let _ = git::run(
        clone,
        &[
            "update-ref",
            &format!("refs/remotes/origin/{remote_branch}"),
            "HEAD",
        ],
        git::READ,
    );
    attempt.branch = Some(remote_branch.clone());
    say(Say::Done(
        format!("Pushed {remote_branch} of {at} via {}.", forge.kind.cli()).into(),
    ));
    let Some(base) = base else {
        attempt.outcome = "saved";
        return 0;
    };
    attempt.outcome = "pushed";

    let repo_on_forge = match pr::Repo::of_clone(request.plane, clone) {
        Ok(found) => found,
        Err(why) => {
            say(Say::Fail(format!("Pushed, but no {noun}: {why}.").into()));
            attempt.outcome = "blocked";
            attempt.detail = why;
            return 1;
        }
    };
    let title = git::run(clone, &["log", "-1", "--format=%s"], git::READ)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    let body = format!(
        "Saved by {} from the {} workspace ([repos.{}] mode = {}).\n\n{}",
        // The forge keeps it: the program as the plane spells it (D-RN11a-4).
        crate::names::BINARY.writes_for(request.plane),
        request.workspace,
        request.name,
        mode.as_str(),
        pr::MARKER
    );
    let opened = match repo_on_forge.backend().open_or_update(
        &trigger.caller(),
        &repo_on_forge.path,
        &remote_branch,
        &base,
        &title,
        &body,
    ) {
        Ok(opened) => opened,
        Err(why) => {
            say(Say::Fail(
                format!("Pushed {remote_branch}, but the {noun} into {base} was not opened: {why}")
                    .into(),
            ));
            attempt.detail = why.to_string();
            return 1;
        }
    };
    attempt.outcome = "pr-open";
    attempt.pr = Some(opened.pr.url.clone());
    if !opened.ours {
        // Somebody's own PR from this branch: its title, its body and whether it merges are
        // theirs. The push has already reached it, which is what a save of their branch is.
        let said = format!(
            "{remote_branch} already has a {noun} purlis did not open, {}: {} — \
             purlis left its title, description and merging alone",
            forge.kind.request_ref(opened.pr.number),
            opened.pr.url
        );
        say(Say::Info(said.clone().into()));
        attempt.detail = said;
        return 0;
    }
    let opened = opened.pr;
    say(Say::Done(
        format!(
            "{} {} from {remote_branch} into {base}: {}",
            forge.kind.request_noun_capitalised(),
            forge.kind.request_ref(opened.number),
            opened.url
        )
        .into(),
    ));
    attempt.number = Some(opened.number);
    if mode == Mode::PrMerge {
        // purlis merges it itself, and not now: this commit reached the forge a moment ago,
        // and no check on it has had time to start (ADR 0051, amended 2026-10-13). The app
        // looks again by itself ([`look`]), and so does the next save.
        let waits = Merging::new(MergeState::Waiting, pr::NOT_STARTED_YET);
        let request = format!(
            "{} {}",
            forge.kind.request_noun(),
            forge.kind.request_ref(opened.number)
        );
        let said = waits.said(&request, &opened.url);
        say(Say::Info(format!("{said}.").into()));
        attempt.detail = said;
        attempt.merge = Some(waits);
    }
    0
}

/// A `pr-merge` repo's open save request, as its newest journal line keeps it.
struct OpenRequest {
    line: serde_json::Value,
    pr: pr::Pr,
    /// The commit the save pushed, which is still the clone's HEAD.
    head: String,
    /// When the save pushed that commit, in seconds since the epoch: the line's `pushed_at`,
    /// which every later line about the same request carries on unchanged, so a look never
    /// measures the wait from itself.
    at: Option<u64>,
    merge: Merging,
}

/// The request of `repo` purlis may still merge: its newest journal line opened or updated one
/// in `pr-merge`, the clone is still on the commit that line pushed, and the line keeps where
/// the request stands. Read from the journal and git, never the network.
fn open_request(plane: &Path, workspace: &str, repo: &repos::Repo) -> Option<OpenRequest> {
    if planesave::Settings::read(plane).repo(&repo.name).mode.value != Mode::PrMerge {
        return None;
    }
    let line = last_entry(plane, workspace, &repo.name)?;
    let text = |key: &str| line.get(key).and_then(serde_json::Value::as_str);
    if text("outcome") != Some("pr-open") {
        return None;
    }
    let head = text("commit")
        .filter(|sha| forge::checks::sha_ok(sha))?
        .to_string();
    if rev(&repo.path, "HEAD").as_deref() != Some(head.as_str()) {
        return None;
    }
    let merge = line.get("merge").and_then(Merging::from_json)?;
    let number = line.get("number").and_then(serde_json::Value::as_u64)?;
    let url = text("pr").unwrap_or_default().to_string();
    let at = line.get("pushed_at").and_then(serde_json::Value::as_u64);
    Some(OpenRequest {
        pr: pr::Pr { number, url },
        head,
        at,
        merge,
        line,
    })
}

/// Whether `repo` has a save request purlis is to look at again, and when its head was pushed
/// (seconds since the epoch) with that head: what the app paces its looks by. A request in a
/// merge queue or already merged is not looked at.
pub fn request_to_look_at(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
) -> Option<(Option<u64>, String)> {
    open_request(plane, workspace, repo)
        .filter(|open| !matches!(open.merge.state, MergeState::Queued | MergeState::Merged))
        .map(|open| (open.at, open.head))
}

/// The branch a save of `repo` pushes and the branch its request goes into, **as the settings
/// and the clone say now**: what the forge's answer about the request must match.
///
/// The base is `[repos.<name>] branch`, else the repo's default branch. The head is the branch
/// the clone is on; or, when that is the base or the default branch, which a request mode
/// never pushes, the branch of purlis's own the save made for it: named by the journal line,
/// and taken only when it is `purlis/<workspace>/…` (under any prefix the product has had)
/// and stands at the clone's HEAD.
fn saves_branch_and_base(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    line: &serde_json::Value,
) -> Result<(String, String), String> {
    let at = format!("{workspace}/{}", repo.name);
    let settings = planesave::Settings::read(plane).repo(&repo.name);
    let default = default_branch(plane, &repo.name, &repo.path);
    let base = settings
        .branch
        .value
        .clone()
        .or_else(|| default.clone())
        .ok_or_else(|| format!("purlis does not know where a request of {at} goes"))?;
    let on = match repos::state_of(&repo.path).map(|state| state.head) {
        Ok(Head::Branch(on)) => on,
        _ => return Err(format!("{at} is not on a branch")),
    };
    if on != base && default.as_deref() != Some(on.as_str()) {
        return Ok((on, base));
    }
    let own = line
        .get("branch")
        .and_then(serde_json::Value::as_str)
        .filter(|name| planesave::branch_ok(name))
        .filter(|name| {
            crate::names::BRANCH_PREFIX
                .spellings()
                .any(|prefix| name.starts_with(&format!("{prefix}{workspace}/")))
        })
        .filter(|name| {
            let tip = rev(&repo.path, &format!("refs/heads/{name}"));
            tip.is_some() && tip == rev(&repo.path, "HEAD")
        });
    match own {
        Some(own) => Ok((own.to_owned(), base)),
        None => Err(format!(
            "{at} is on {on}, which a request mode never pushes, and no branch of purlis's own \
             for this workspace stands at its commit"
        )),
    }
}

/// Ask the forge about `open` under `gate`, and write what it found as the repo's newest
/// journal line when that differs from what the line before kept. Answers what was found.
fn ask_and_keep(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    open: &OpenRequest,
    gate: pr::Gate,
    caller: &forge::Caller,
    trigger: &str,
) -> Result<(Merging, bool), String> {
    let on_forge = pr::Repo::of_clone(plane, &repo.path)?;
    let (branch, base) = saves_branch_and_base(plane, workspace, repo, &open.line)?;
    let at = pr::Pinned {
        sha: &open.head,
        branch: &branch,
        base: &base,
    };
    let looked = on_forge
        .backend()
        .merge_when_ready(caller, &on_forge.path, &open.pr, &at, gate);
    if gate == pr::Gate::Person
        && let Err(why) = &looked
    {
        return Err(why.to_string());
    }
    let merging = Merging::of_a_look(&looked, &on_forge.forge, || {
        forge::signin::ready(&on_forge.forge, caller)
    });
    let now = planegit::now() as u64;
    // A stamp that says nothing (none, or one in the future after a clock was set back) is
    // written again as now, so the head settles a wait later rather than never.
    let restamp = planegit::stamp_is_unusable(open.at, now);
    let changed = merging != open.merge;
    if changed || restamp {
        let request = format!(
            "{} {}",
            on_forge.forge.kind.request_noun(),
            on_forge.forge.kind.request_ref(open.pr.number)
        );
        let mut line = open.line.clone();
        line["at"] = serde_json::json!(planegit::now());
        line["trigger"] = serde_json::json!(trigger);
        line["files"] = serde_json::json!(0);
        line["ms"] = serde_json::json!(0);
        line["merge"] = merging.to_json();
        if restamp {
            line["pushed_at"] = serde_json::json!(now);
        }
        line["detail"] = serde_json::json!(merging.said(&request, &open.pr.url));
        if merging.state == MergeState::Merged {
            line["outcome"] = serde_json::json!("pushed");
        }
        planegit::journal_append(plane, &line);
        // The journal moved and no git file did: the shared standing is told.
        planegit::touch(&repo.path);
    }
    Ok((merging, changed))
}

/// **Look again at a `pr-merge` repo's open save request**, with no save: merge it if every
/// check has passed since, or it has been there long enough and nothing says a check will run;
/// otherwise keep where it stands on the journal for the row to say. The repo's half of
/// [`planegit::look`]. It touches neither the clone's tree nor its refs, so it does not wait
/// for a chat mid-turn.
pub fn look(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    caller: &forge::Caller,
) -> Result<Looked, String> {
    let Some(_claim) = Claim::of(&repo.path) else {
        return Err(already_saving(workspace, &repo.name));
    };
    let Some(open) = open_request(plane, workspace, repo) else {
        return Ok(Looked::default());
    };
    if matches!(open.merge.state, MergeState::Queued | MergeState::Merged) {
        return Ok(Looked::default());
    }
    // What the journal keeps can only make the look wait: the backend holds a settled head to
    // the forge's own clock, and asks the forge whose request this is.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let gate = pr::Gate::Checks {
        settled: planegit::settled_at(open.at, now),
    };
    let (merging, changed) = ask_and_keep(plane, workspace, repo, &open, gate, caller, "look")?;
    Ok(Looked {
        open: !matches!(merging.state, MergeState::Queued | MergeState::Merged),
        changed,
        moved: false,
    })
}

/// **What the window's Fix hands a chat about a repo's red request**: the repo's half of
/// [`planegit::trouble`]. Refused unless the last look found it red.
pub fn trouble(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    caller: &forge::Caller,
) -> Result<(u64, crate::savefix::Trouble), String> {
    let red =
        open_request(plane, workspace, repo).filter(|open| open.merge.state == MergeState::Red);
    let Some(open) = red else {
        return Err(format!(
            "{workspace}/{} has no red pull request of a pr-merge save at the commit it is on, \
             so there is nothing to fix",
            repo.name
        ));
    };
    let on_forge = pr::Repo::of_clone(plane, &repo.path)?;
    let kind = on_forge.forge.kind;
    let request = format!(
        "{} {}",
        kind.request_noun(),
        kind.request_ref(open.pr.number)
    );
    let failed = crate::savefix::read_failed(
        on_forge.backend().as_ref(),
        caller,
        &on_forge.path,
        &open.head,
        open.pr.number,
        &open.merge.checks,
    )
    .map_err(|not| not.said(&request))?;
    Ok((
        open.pr.number,
        crate::savefix::red_request(
            &request,
            &open.pr.url,
            &on_forge,
            crate::savefix::Whose::Repo {
                workspace,
                name: &repo.name,
            },
            failed,
        ),
    ))
}

/// The commit a repo's open request is at and where it stands, as its journal keeps them:
/// the repo's half of [`planegit::seen`].
pub fn seen(plane: &Path, workspace: &str, repo: &repos::Repo) -> Option<planegit::Seen> {
    let open = open_request(plane, workspace, repo)?;
    Some(planegit::Seen {
        shown: open.merge.shown(),
        head: open.head,
    })
}

/// **The person's merge of a repo's request, of what they were shown**: the repo's half of
/// [`planegit::merge_as_seen`]. A fresh look first; merged as it stands only at the commit
/// and in the standing the confirmation showed.
pub fn merge_as_seen(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    caller: &forge::Caller,
    seen: &planegit::Seen,
) -> Result<MergedAsItStands, String> {
    look(plane, workspace, repo, caller)?;
    let Some(open) = open_request(plane, workspace, repo) else {
        return Err(planegit::changed_since(seen, "", None).unwrap_or_default());
    };
    match open.merge.state {
        MergeState::Merged => {
            return Ok(MergedAsItStands::Later(
                "Every check on it passed, and purlis merged it as its checks stand.".into(),
            ));
        }
        MergeState::Queued => return Ok(MergedAsItStands::Later(open.merge.why)),
        _ => {}
    }
    if let Some(changed) = planegit::changed_since(seen, &open.head, Some(&open.merge)) {
        return Err(changed);
    }
    merge_as_it_stands(plane, workspace, repo, caller, seen)
}

/// **Merge a repo's open save request as it stands**, its failed and missing checks included:
/// the person's own action in the window, and only there (the operator's ruling P1,
/// 2026-10-13). The repo's half of the plane's merge as it stands. The clone is not moved:
/// a repo's history is its developers' (ADR 0051).
///
/// Private, and reached only from [`merge_as_seen`] (#1978): the press that showed the
/// request's checks. `seen` is what that press showed, and it is checked again under this
/// merge's own claim: a request at another commit by now, or one whose standing changed since
/// (its checks failed after the look), is refused before the forge is asked.
fn merge_as_it_stands(
    plane: &Path,
    workspace: &str,
    repo: &repos::Repo,
    caller: &forge::Caller,
    seen: &planegit::Seen,
) -> Result<MergedAsItStands, String> {
    let Some(_claim) = Claim::of(&repo.path) else {
        return Err(already_saving(workspace, &repo.name));
    };
    let Some(open) = open_request(plane, workspace, repo) else {
        return Err(format!(
            "{workspace}/{} has no open pull request of a pr-merge save at the commit it is on",
            repo.name
        ));
    };
    if let Some(changed) = planegit::changed_since(seen, &open.head, Some(&open.merge)) {
        return Err(changed);
    }
    let (merging, _) = ask_and_keep(
        plane,
        workspace,
        repo,
        &open,
        pr::Gate::Person,
        caller,
        "merge",
    )?;
    match merging.state {
        MergeState::Merged => Ok(MergedAsItStands::Later(format!(
            "Merged as it stands, as you asked: {}",
            open.pr.url
        ))),
        MergeState::Queued => Ok(MergedAsItStands::Later(merging.why)),
        _ => Err(format!("It was not merged: {}", merging.why)),
    }
}

/// The branch a request-mode save pushes when the clone stands on its PR's base or its default
/// branch: one an earlier save of this repo pushed, while HEAD still descends from it, else
/// a fresh `purlis/<workspace>/<short-sha>` at HEAD. Created or moved forward locally too, so
/// the operator can see it; never moved backwards or sideways.
///
/// A branch carried on may be under any prefix the product has used: a `charter/<workspace>/…`
/// branch made before the rename keeps its open pull request (V93j). Only a fresh one is
/// named with the purlis prefix.
fn save_branch(request: &Request, on: &str, head_sha: &str) -> Result<String, String> {
    let clone = request.clone;
    let prefixes: Vec<String> = crate::names::BRANCH_PREFIX
        .spellings()
        .map(|p| format!("{p}{}/", request.workspace))
        .collect();
    let ours = |name: &str| prefixes.iter().any(|p| name.starts_with(p.as_str()));
    let prefix = &prefixes[0];
    // The newest journal line that NAMES one — a quit's commit-only line or a failed push
    // names none, and must not break the chain — then every local branch charter made here,
    // newest first. The first that HEAD still descends from carries on.
    let wanted = target(request.workspace, request.name);
    let journalled = planegit::journal(request.plane)
        .into_iter()
        .rev()
        .filter(|line| line.get("target").and_then(serde_json::Value::as_str) == Some(&wanted))
        .find_map(|line| line.get("branch")?.as_str().map(str::to_owned))
        .filter(|name| ours(name));
    let mut list = vec![
        "for-each-ref".to_string(),
        "--sort=-committerdate".into(),
        "--format=%(refname:short)".into(),
    ];
    list.extend(prefixes.iter().map(|p| format!("refs/heads/{p}")));
    let list: Vec<&str> = list.iter().map(String::as_str).collect();
    let local = git::run(clone, &list, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.out.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let reuse = journalled
        .into_iter()
        .chain(local)
        .filter(|name| ours(name) && planesave::branch_ok(name))
        .find(|name| {
            git::run(
                clone,
                &[
                    "merge-base",
                    "--is-ancestor",
                    &format!("refs/heads/{name}"),
                    "HEAD",
                ],
                git::READ,
            )
            .is_ok_and(|r| r.ok())
        });
    let name = match reuse {
        Some(name) => name,
        None => {
            let short = &head_sha[..head_sha.len().min(7)];
            let name = format!("{prefix}{short}");
            if !planesave::branch_ok(&name) {
                return Err(format!(
                    "{name} is not a branch name git would accept, so purlis cannot move \
                     {on}'s commits off {on}"
                ));
            }
            name
        }
    };
    let moved = git::run(
        clone,
        &["update-ref", &format!("refs/heads/{name}"), "HEAD"],
        git::READ,
    );
    if !moved.as_ref().is_ok_and(git::Run::ok) {
        let said = moved.map_or_else(|e| e.to_string(), |run| planegit::tail(&run));
        return Err(format!("purlis could not create {name}: {said}"));
    }
    Ok(name)
}

/// The names a file of credentials goes by, whatever is in it.
pub(crate) fn secret_name(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    let example = [".example", ".sample", ".template", ".dist"]
        .iter()
        .any(|suffix| lower.ends_with(suffix));
    if lower == ".env" || (lower.starts_with(".env.") && !example) {
        return Some("an environment file");
    }
    if ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"].contains(&lower.as_str()) {
        return Some("an SSH private key");
    }
    if [".pem", ".p12", ".pfx", ".key"]
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        return Some("a key or certificate store");
    }
    if lower == "credentials.json" {
        return Some("a credentials file");
    }
    None
}

/// The staged files of `staged` a repo save refuses, each with what it looks like — **the
/// narrower rule a repo takes** (ADR 0051). The plane's own guard reads its memory and ref
/// files with a credential-assignment rule that ordinary code trips (`password: String`), so a
/// clone is asked two things instead:
///
/// - a file whose NAME is a credential's: `.env` and `.env.*` (not `.example`, `.sample`,
///   `.template`, `.dist`), an SSH private key, `*.pem`, `*.p12`, `*.pfx`, `*.key`,
///   `credentials.json`, and a `.npmrc` or `.pypirc` that holds a token or a password;
/// - a private key block or a live token by its forge's own prefix in any file's staged text,
///   as written and with its escapes decoded ([`crate::secretshape::token_kind_as_read`],
///   #1304), since a string can spell a character as an escape.
///
/// Asked of the staged blob (`git show :<path>`), as the plane's guard asks it. What was found
/// is named by path and kind, never by value.
fn secret_files(clone: &Path, staged: &[String]) -> Vec<(String, &'static str)> {
    staged
        .iter()
        .filter_map(|path| {
            // A deletion is staged too, and taking a secret out is not what this refuses.
            let blob = git::run(clone, &["show", &format!(":0:{path}")], git::READ)
                .ok()
                .filter(git::Run::ok)?;
            refused_as(path, &blob.out).map(|kind| (path.clone(), kind))
        })
        .collect()
}

/// **What the file at `path`, holding `text`, looks like where a save to the forge refuses
/// it**, or none: [`secret_files`]'s rule for one file, which a send of uncommitted work to a
/// runner follows too (ADR 0078 §7, the operator's ruling of 2026-10-25).
pub(crate) fn refused_as(path: &str, text: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some(kind) = secret_name(name) {
        return Some(kind);
    }
    let lower = name.to_ascii_lowercase();
    if (lower == ".npmrc" || lower == ".pypirc")
        && text.lines().any(|line| {
            let line = line.to_ascii_lowercase();
            (line.contains("_authtoken") || line.contains("password")) && line.contains('=')
        })
    {
        return Some("a package registry token");
    }
    crate::secretshape::token_kind_as_read(text)
}

/// The repo's default branch: its `default_branch` in `inventory/repos.json`, else what the
/// clone's `origin/HEAD` names.
pub fn default_branch(plane: &Path, name: &str, clone: &Path) -> Option<String> {
    let listed = crate::inventory::load(plane, "")
        .ok()
        .map(|doc| crate::inventory::listed(&doc))
        .unwrap_or_default();
    let recorded = crate::inventory::find(&listed, name)
        .and_then(|record| record.get("default_branch")?.as_str())
        .filter(|b| planesave::branch_ok(b))
        .map(str::to_owned);
    recorded.or_else(|| {
        git::run(
            clone,
            &[
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ],
            git::READ,
        )
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .and_then(|head| head.strip_prefix("origin/").map(str::to_owned))
        .filter(|b| planesave::branch_ok(b))
    })
}

/// How many files a generated message names before it says how many more there are.
const SUMMARY_FILES: usize = 5;

/// The message a repo save writes when it is given none: how many files, and which —
/// `charter save: 2 files (src/a.rs, README.md)`. `program` is the program as the plane spells
/// it in what it writes (D-RN11a-4).
fn summary(staged: &[String], program: &str) -> String {
    let mut named: Vec<String> = staged.iter().take(SUMMARY_FILES).cloned().collect();
    if staged.len() > SUMMARY_FILES {
        named.push(format!("+{} more", staged.len() - SUMMARY_FILES));
    }
    let files = if staged.len() == 1 { "file" } else { "files" };
    format!(
        "{program} save: {} {files} ({})",
        staged.len(),
        named.join(", ")
    )
}

/// The newest journal line about this repo.
fn last_entry(plane: &Path, workspace: &str, name: &str) -> Option<serde_json::Value> {
    let wanted = target(workspace, name);
    planegit::journal(plane)
        .into_iter()
        .rev()
        .find(|line| line.get("target").and_then(serde_json::Value::as_str) == Some(&wanted))
}

/// Where a repo's unsaved work sits, read from git and the journal — never the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub name: String,
    pub mode: Mode,
    /// The file that decided the mode, or `default`.
    pub mode_from: &'static str,
    pub autosave: bool,
    pub stage: Stage,
    /// The branch the clone is on; `None` when it is on none.
    pub branch: Option<String>,
    /// Files a save would take.
    pub changed: u32,
    /// Commits the remote's copy of the branch lacks; `None` when there is nothing to count
    /// against — a branch never pushed.
    pub ahead: Option<u32>,
    /// The pull request HEAD is waiting on.
    pub pr: Option<String>,
    /// Where that pull request stands with merging, for a `pr-merge` repo: as the plane's
    /// [`planegit::Standing::merge`]. Never a block.
    pub merge: Option<Merging>,
    pub blocked: Option<String>,
    /// HEAD's commit, when there is one.
    pub head: Option<String>,
    /// Whether a save would push: a mode past the commit, and an origin on a known forge.
    pub pushes: bool,
}

impl Standing {
    /// Whether a save of a repo standing like this would do anything.
    pub fn worth_saving(&self) -> bool {
        self.mode != Mode::Off
            && (self.changed > 0 || self.stage == Stage::Blocked || self.stage == Stage::Committed)
    }

    /// What is unsaved, as one string that changes whenever it does — auto-save's quiet
    /// period is measured from its last change.
    pub fn fingerprint(&self) -> String {
        format!("{:?}\0{}\0{:?}", self.head, self.changed, self.ahead)
    }
}

/// Every clone's standing, one per clone, kept as long as a plane's
/// ([`planegit::SHARED_FOR`]).
static CLONES: std::sync::LazyLock<crate::standings::Shared<Standing>> =
    std::sync::LazyLock::new(|| crate::standings::Shared::new(planegit::SHARED_FOR));

/// [`standing`], **shared by every caller in this process** (FD-11, #651): auto-save's look at
/// the repos it saves and the Saving tab's rows read this, and git runs once for both — one
/// computation at a time per clone, again only when the clone's git files move,
/// [`planegit::touch`] says something under it changed, or [`planegit::SHARED_FOR`] has passed. A save decides from
/// [`standing`] itself.
///
/// A clone no watch covers yet asks to be watched ([`crate::standings::want_watched`]), so in
/// the app its standing is read again when its working tree moves rather than on a clock.
pub fn shared_standing(plane: &Path, workspace: &str, repo: &repos::Repo) -> Standing {
    if !CLONES.covered(&repo.path) {
        crate::standings::want_watched(&crate::standings::Wanted {
            plane: plane.to_path_buf(),
            workspace: workspace.to_string(),
            repo: repo.name.clone(),
            path: repo.path.clone(),
        });
    }
    CLONES.get(&repo.path, || standing(plane, workspace, repo))
}

/// A watch now covers the clone at `path` whole, or no longer does ([`crate::standings::Shared::cover`]).
pub fn cover(path: &Path, covered: bool) {
    CLONES.cover(path, covered);
}

/// Whether a watch covers the clone at `path` now.
pub fn covered(path: &Path) -> bool {
    CLONES.covered(path)
}

/// The clones' half of [`planegit::touch`], which is the one to call.
pub(crate) fn touch_within(dir: &Path) {
    CLONES.touch_within(dir);
}

/// Where the clone `repo` of `workspace` stands.
pub fn standing(plane: &Path, workspace: &str, repo: &repos::Repo) -> Standing {
    let settings = planesave::Settings::read(plane).repo(&repo.name);
    let mode = settings.mode.value;
    let mut out = Standing {
        name: repo.name.clone(),
        mode,
        mode_from: settings.mode.source.file().unwrap_or("default"),
        autosave: settings.autosave.value,
        stage: Stage::Saved,
        branch: None,
        changed: 0,
        ahead: None,
        pr: None,
        merge: None,
        blocked: None,
        head: None,
        pushes: false,
    };
    let state = match repos::state_of(&repo.path) {
        Ok(state) => state,
        Err(unreadable) => {
            out.stage = Stage::Blocked;
            out.blocked = Some(unreadable.to_string());
            return out;
        }
    };
    out.changed = state.tracked.saturating_add(state.untracked);
    out.head = rev(&repo.path, "HEAD");
    out.pushes = !matches!(mode, Mode::Off | Mode::Commit)
        && planegit::origin_https_of(plane, &repo.path).is_some();
    if let Head::Branch(branch) = &state.head {
        out.ahead = planegit::count(&repo.path, &format!("refs/remotes/origin/{branch}..HEAD"));
    }
    out.branch = match &state.head {
        Head::Branch(b) | Head::Unborn(b) => Some(b.clone()),
        Head::Detached(_) => None,
    };
    let last = last_entry(plane, workspace, &repo.name);
    let said = |key: &str| {
        last.as_ref()
            .and_then(|l| l.get(key))
            .and_then(serde_json::Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    // A journal line speaks for the clone only while HEAD is still the commit it names.
    let current = said("commit").is_none_or(|sha| Some(sha) == out.head);
    // Git stopped part-way through a merge or rebase, asked of the clone now (#433).
    out.stage = if let Some(stopped) = gitstate::stopped(&repo.path) {
        out.blocked = Some(stopped.why());
        Stage::Blocked
    } else if current
        && said("outcome").as_deref() == Some("blocked")
        && last.as_ref().and_then(|l| l.get("stopped")) != Some(&serde_json::Value::Bool(true))
    {
        out.blocked = said("detail").or_else(|| Some("the last save could not finish".into()));
        Stage::Blocked
    } else if out.changed > 0 {
        Stage::Changed
    } else if !out.pushes {
        Stage::Saved
    } else if current && said("commit").is_some() && said("outcome").as_deref() == Some("pr-open") {
        out.pr = said("pr");
        out.merge = (mode == Mode::PrMerge)
            .then(|| last.as_ref()?.get("merge").and_then(Merging::from_json))
            .flatten();
        Stage::PrOpen
    } else if out.head.is_some() && out.ahead != Some(0) {
        Stage::Committed
    } else {
        Stage::Saved
    };
    out
}

#[cfg(test)]
mod tests;
