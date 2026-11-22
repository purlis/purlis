// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter git-hook <name>`: charter's check in a chat's git hooks (SQ-16).
//!
//! For `pre-commit` and `pre-merge-commit`, the staged diff is scanned
//! ([`purlis_core::diffscan`]); a finding refuses the commit on standard error, masked, and
//! tells the app, so the chat joins the needs-you queue. For `pre-push`, the same scan reads
//! every commit the push would send that the remote does not have (SQ-7,
//! [`purlis_core::diffscan::pushed`]), and refuses the same way, and refuses a push that would
//! publish a change to the scan's allowlist. A chat's `pre-commit` also refuses a staged
//! project file whose `[sandbox]`, `[chat_env]` or `[dispatch]` differ from the file in the
//! working tree (#1464). For `commit-msg`, a message that holds what looks like a credential is
//! refused the same way (#1133, [`purlis_core::diffscan::message_checked`]); otherwise it is
//! stamped with the chat's provenance trailers ([`purlis_core::provenance::stamp`]) and the hook
//! succeeds, stamped or not. Every other hook has no check and answers success. The repository's own hook of the
//! same name is the shim's to run, after this ([`purlis_core::githooks`]).

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use purlis_core::diffscan::Stopped;
use purlis_core::githooks::{CHECKED, COMMIT_MSG, MESSAGE_REFUSED, PRE_COMMIT, PRE_PUSH};
use purlis_core::{diffscan, hookwire};

/// Runs charter's check for hook `name` in the repository git ran it in: this process's
/// working directory, which git sets to the top of the work tree.
pub fn run(name: &str, args: &[String]) -> ExitCode {
    if name == COMMIT_MSG {
        return message(args);
    }
    if name == PRE_PUSH {
        return push(args);
    }
    if !CHECKED.contains(&name) {
        return ExitCode::SUCCESS;
    }
    let Ok(repo) = std::env::current_dir() else {
        eprintln!("purlis: this commit could not be scanned for secrets (no working directory)");
        return ExitCode::FAILURE;
    };
    let mut scan = match diffscan::checked(&repo) {
        Ok(scan) => scan,
        Err(why) => {
            eprintln!("purlis: this commit could not be scanned for secrets ({why})");
            return ExitCode::FAILURE;
        }
    };
    // A merge brings the allowlist as it was committed on the other side, by whoever committed
    // it there; only a commit a chat makes itself may not change it (SQ-17).
    if name == PRE_COMMIT && scan.changes_the_allowlist {
        eprint!("{}", diffscan::ALLOWLIST_REFUSAL);
        tell_the_app(format!(
            "commit refused in {}: it changes the scan's allowlist",
            named(&repo)
        ));
        return ExitCode::FAILURE;
    }
    // A change to what chats run under staged without the file in the working tree (#1464):
    // only a commit a chat makes itself is held to it, as for the allowlist.
    if name == PRE_COMMIT && scan.stages_what_a_chat_runs_under {
        eprint!("{}", diffscan::RUNS_UNDER_REFUSAL);
        tell_the_app(format!(
            "commit refused in {}: it stages a change to the project's [sandbox], [chat_env] \
             or [dispatch] that the file in the working tree does not hold",
            named(&repo)
        ));
        return ExitCode::FAILURE;
    }
    under_policy(&mut scan.refused, &repo);
    if scan.refused.is_empty() {
        return ExitCode::SUCCESS;
    }
    refuse(Stopped::Commit, &repo, &scan)
}

/// **What the scan refused, as the project's `[policy]` runs each finding's family** (Q308a):
/// `guards.credential` for a credential, `guards.save` for personal data, read for the chat these
/// hooks run in, as the app answers it ([`purlis_core::enforcement::in_force`]).
fn under_policy(refused: &mut Vec<diffscan::Finding>, repo: &Path) {
    let root = purlis_core::plane::resolve(repo).unwrap_or_else(|_| repo.to_path_buf());
    let policy = purlis_core::enforcement::in_force(&root);
    let chat = purlis_core::enforcement::this_chat();
    diffscan::under_policy(refused, &root, chat.as_deref(), &policy);
}

/// `commit-msg`: git's argument names the file holding the message it is about to commit. A
/// message that holds what looks like a credential is refused (#1133), with
/// [`MESSAGE_REFUSED`], the one status the shim refuses on; one this cannot read is refused the
/// same way, as a commit nothing scanned is. Otherwise the message is stamped with the chat's
/// provenance trailers, and a message that could not be stamped is committed as written.
fn message(args: &[String]) -> ExitCode {
    let cannot = |why: &str| {
        eprintln!("purlis: this commit's message could not be scanned for secrets ({why})");
        ExitCode::from(MESSAGE_REFUSED)
    };
    let Ok(top) = std::env::current_dir() else {
        return cannot("no working directory");
    };
    let Some(file) = args.first() else {
        return cannot("git named no message file");
    };
    let file = top.join(file);
    let said = match std::fs::read(&file) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => return cannot(&e.to_string()),
    };
    let mut scan = diffscan::message_checked(&top, &said);
    under_policy(&mut scan.refused, &top);
    if !scan.refused.is_empty() {
        say_refused(Stopped::Commit, &top, &scan);
        return ExitCode::from(MESSAGE_REFUSED);
    }
    purlis_core::provenance::stamp(&file, &top, &purlis_core::envvar::var);
    ExitCode::SUCCESS
}

/// `pre-push`: git's arguments are the remote's name and its URL, and standard input has one
/// line per ref the push updates. Anything charter cannot read refuses the push, as a commit it
/// could not scan is refused.
fn push(args: &[String]) -> ExitCode {
    let cannot = |why: &str| {
        eprintln!("purlis: this push could not be scanned for secrets ({why})");
        ExitCode::FAILURE
    };
    let Ok(repo) = std::env::current_dir() else {
        return cannot("no working directory");
    };
    let [_remote, url, ..] = args else {
        return cannot("git named no remote");
    };
    let mut updates = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut updates) {
        return cannot(&format!("the refs git sent could not be read: {e}"));
    }
    // Kept until the scan is done: the route's authority is a file in it.
    let held = std::cell::RefCell::new(None);
    match diffscan::pushed_through(&repo, url, &updates, &|| chat_route(&held)).map(|mut scan| {
        under_policy(&mut scan.refused, &repo);
        scan
    }) {
        Ok(scan) if scan.changes_the_allowlist => {
            eprint!("{}", diffscan::PUSH_ALLOWLIST_REFUSAL);
            tell_the_app(format!(
                "push refused in {}: it changes the scan's allowlist",
                named(&repo)
            ));
            ExitCode::FAILURE
        }
        Ok(scan) if scan.refused.is_empty() => ExitCode::SUCCESS,
        Ok(scan) => refuse(Stopped::Push, &repo, &scan),
        Err(why) => cannot(&why),
    }
}

/// How long the app has to answer for the chat's route: it answers from what it holds.
const ROUTE_ANSWERED_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// **The route this chat's git reaches its forge by, for the scan's ask of the remote** (#7302):
/// under a chat's one login its git goes only through the app's proxy, under the chat's own
/// authority and with its sentinel, and an ask made any other way fails and reads the whole push.
///
/// The authority, the credential helper and the hosts are **the app's**, from its own record of
/// the chat ([`Ask::GitRoute`]), never the environment's or a repository's config: what a chat
/// can write could name an authority that answers for the remote. The one thing taken from the
/// environment is the chat's own egress port, a proxy that reads nothing a certificate under the
/// app's authority does not let it read; with the sentinel, which only that proxy is shown. `None`
/// where the app does not answer, the chat has no route, or any part is unusable: the remote is
/// then asked as before and, if it cannot be, the push is read in full.
fn chat_route(
    held: &std::cell::RefCell<Option<OwnFolder>>,
) -> Option<purlis_core::worktree::git::NetworkRoute> {
    use purlis_core::hookwire::{Answer, Ask, GitRouteAsk};
    let env = purlis_core::envvar::var;
    let proxy = env("HTTPS_PROXY").or_else(|| env("https_proxy"))?;
    let sentinel = env(purlis_core::forge::gitcredential::SENTINEL_ENV)?;
    let Ok(Ok(Answer::GitRoute { route })) = crate::brokered::asked(
        |chat| Ask::GitRoute(GitRouteAsk { chat }),
        ROUTE_ANSWERED_WITHIN,
    ) else {
        return None;
    };
    let dir = OwnFolder::made()?;
    let ca = dir.0.join("ca.pem");
    std::fs::write(&ca, &route.ca_pem).ok()?;
    let found = purlis_core::forgewire::git_network_in_hook(&route, &proxy, &ca, &sentinel).ok();
    *held.borrow_mut() = Some(dir);
    found
}

/// A folder of this hook's own, made new (never one that is there already, a link included) and
/// closed to others, and removed when it is dropped.
struct OwnFolder(std::path::PathBuf);

impl OwnFolder {
    fn made() -> Option<Self> {
        use std::os::unix::fs::DirBuilderExt as _;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path =
            std::env::temp_dir().join(format!("purlis-route-{}-{nanos}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&path).ok()?;
        Some(Self(path))
    }
}

impl Drop for OwnFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The repository as a needs-you item names it: its directory's name.
fn named(repo: &Path) -> String {
    repo.file_name().map_or_else(
        || repo.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Says what `scan` refused, masked, on standard error, tells the app, and fails.
fn refuse(stopped: Stopped, repo: &Path, scan: &diffscan::Scan) -> ExitCode {
    say_refused(stopped, repo, scan);
    ExitCode::FAILURE
}

/// Says what `scan` refused, masked, on standard error, and tells the app, for a caller that
/// fails with an exit code of its own.
fn say_refused(stopped: Stopped, repo: &Path, scan: &diffscan::Scan) {
    eprint!("{}", diffscan::refusal(stopped, &scan.refused));
    for problem in &scan.problems {
        eprintln!("  (and {problem})");
    }
    tell_the_app(diffscan::summary(stopped, repo, &scan.refused));
}

/// The app's needs-you item for this refusal. The commit is refused whatever this answers, and
/// a chat outside the app has none. A line the app does not take is spooled for the next host
/// to record (ADR 0068 §6, FD-30).
fn tell_the_app(why: String) {
    let env = purlis_core::envvar::var;
    let (Some(socket), Some(chat)) = (
        env(hookwire::SOCKET_ENV),
        env(hookwire::CHAT_ENV).and_then(|chat| chat.parse().ok()),
    ) else {
        return;
    };
    let _ = hookwire::deliver_refused(
        Path::new(&socket),
        hookwire::ChatToken::from_env().as_ref(),
        &hookwire::CommitRefused {
            chat,
            commit_refused: why,
        },
    );
}
