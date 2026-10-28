// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter report bug|feature`, asked of the binary with a stand-in `gh` on `PATH` that writes
//! down every call. No test here reaches GitHub.
//!
//! What is pinned: a run shows the draft; only `--yes <digest>` naming the draft that was shown
//! goes on to file it; nothing is ever asked of `gh`, whatever login or token it would use
//! (#806): a report is searched and filed with purlis's own sign-in, which a command does not
//! hold, so the command shows the prefilled link instead; what that link carries is scrubbed;
//! and a saved panic becomes a draft bug.

use crate::support;

use std::path::PathBuf;
use std::process::Output;

struct World {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    bin: PathBuf,
}

impl World {
    /// A plane, a home, and a stand-in `gh` on `PATH` that writes down every call it is asked
    /// and answers as a `gh` that filed would, so a report that reached for it would pass as
    /// filed if the log did not catch it.
    fn new() -> World {
        let tmp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(tmp.path()).unwrap();
        let root = base.join("plane");
        let home = base.join("home");
        let bin = base.join("bin");
        for dir in [
            &root.join("workspaces/acme-rollout/billing-api/.git"),
            &home,
            &bin,
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
        let log = base.join("gh-calls.log");
        let script = format!(
            "#!/bin/sh\n\
             echo '=== call' >> '{log}'\n\
             for a in \"$@\"; do printf '%s\\n' \"$a\" >> '{log}'; done\n\
             echo 'https://github.com/purlis/purlis/issues/999'\n\
             exit 0\n",
            log = log.display(),
        );
        stand_in::program(&bin, "gh", &script);
        World {
            _tmp: tmp,
            root,
            home,
            bin,
        }
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.bin.parent().unwrap().join("gh-calls.log")).unwrap_or_default()
    }

    fn charter(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        support::purlis()
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("CHARTER_PANIC_LOG", self.home.join("no-panics.log"))
            .envs(env.iter().copied())
            .output()
            .expect("charter runs")
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The digest of the draft `args` make: what a `--yes` naming another one is told.
fn digest(w: &World, args: &[&str], env: &[(&str, &str)]) -> String {
    let o = w.charter(&[args, &["--yes", "000000000000"]].concat(), env);
    let text = err(&o);
    let at = text.find("its digest is ").expect("the digest is named") + "its digest is ".len();
    text[at..at + 12].to_string()
}

/// The sentence a command is refused with: it holds no purlis sign-in.
const NO_SIGN_IN: &str = "purlis sends this only with its own sign-in to GitHub on github.com";

#[test]
fn a_bare_run_shows_the_draft_and_the_link_and_asks_gh_nothing() {
    let w = World::new();
    let o = w.charter(
        &["report", "bug", "The status line is wrong\n\nIt says 3."],
        &[],
    );

    assert!(o.status.success(), "{}", err(&o));
    let text = out(&o);
    assert!(text.contains("nothing has been filed"), "{text}");
    assert!(text.contains("Title: The status line is wrong"), "{text}");
    assert!(text.contains("It says 3."), "{text}");
    assert!(text.contains(NO_SIGN_IN), "{text}");
    assert!(
        text.contains(
            "https://github.com/purlis/purlis/issues/new?title=The%20status%20line%20is%20wrong"
        ),
        "{text}"
    );
    assert!(w.calls().is_empty(), "gh was asked: {}", w.calls());
}

#[test]
fn yes_with_the_digest_that_was_shown_is_refused_with_the_link_and_never_asks_gh() {
    let w = World::new();
    let args = ["report", "feature", "charter should report its own bugs"];
    let d = digest(&w, &args, &[]);

    let o = w.charter(
        &[&args[..], &["--yes", &d]].concat(),
        &[("GH_TOKEN", "a-plane-bot-token-value")],
    );

    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    assert!(err(&o).contains("purlis did not file it"), "{}", err(&o));
    assert!(err(&o).contains(NO_SIGN_IN), "{}", err(&o));
    assert!(
        err(&o).contains("https://github.com/purlis/purlis/issues/new?title=charter%20should"),
        "{}",
        err(&o)
    );
    assert!(w.calls().is_empty(), "gh was asked: {}", w.calls());
}

#[test]
fn yes_with_a_digest_that_is_not_this_drafts_sends_nothing() {
    let w = World::new();
    let o = w.charter(
        &["report", "bug", "something broke", "--yes", "000000000000"],
        &[],
    );
    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    assert!(err(&o).contains("not the draft"), "{}", err(&o));
    assert!(w.calls().is_empty(), "{}", w.calls());
}

#[test]
fn a_secret_a_vault_value_or_a_private_path_never_reaches_the_draft_or_its_link() {
    let w = World::new();
    let token = format!("ghp_{}", "Zz9Yy8Xx".repeat(5));
    let body = format!(
        "sync failed in acme-rollout for billing-api\n\
         value was vault-value-7f3a9c\n\
         {token}\n\
         at {}/workspaces/acme-rollout/x",
        w.root.display()
    );
    let file = w.home.join("report.md");
    std::fs::write(&file, &body).unwrap();
    let args = ["report", "bug", "--from-file", file.to_str().unwrap()];
    let env = [("DEPLOY_KEY", "vault-value-7f3a9c")];
    let d = digest(&w, &args, &env);
    let o = w.charter(&[&args[..], &["--yes", &d]].concat(), &env);

    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    let shown = format!("{}{}", out(&o), err(&o));
    for leak in [
        "acme-rollout",
        "billing-api",
        "vault-value-7f3a9c",
        token.as_str(),
        w.root.to_str().unwrap(),
    ] {
        assert!(!shown.contains(leak), "{leak} was shown or linked: {shown}");
    }
    assert!(shown.contains("[env $DEPLOY_KEY]"), "{shown}");
    assert!(
        shown.contains("sync failed in [workspace] for [repo]"),
        "{shown}"
    );
    assert!(w.calls().is_empty(), "gh was asked: {}", w.calls());
}

#[test]
fn a_saved_panic_is_offered_and_becomes_a_draft_bug_with_its_place_and_version() {
    let w = World::new();
    let log = w.home.join("panics.log");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        &log,
        format!(
            "charter-panic pid 4 at {now} (seconds since 1970)\n\
         version: 1.2.3\n\
         thread: main\n\
         place: crates/purlis-core/src/engine.rs:88:9\n\
         message: index out of bounds\n\
         backtrace:\n   0: somewhere\n\
         charter-panic end\n"
        ),
    )
    .unwrap();
    let env = [("CHARTER_PANIC_LOG", log.to_str().unwrap())];

    let bare = w.charter(&["report", "bug"], &env);
    assert_eq!(bare.status.code(), Some(1));
    assert!(
        err(&bare).contains("`charter report bug --panic`"),
        "the saved panic is offered: {}",
        err(&bare)
    );

    let o = w.charter(&["report", "bug", "--panic"], &env);
    assert!(o.status.success(), "{}", err(&o));
    let text = out(&o);
    assert!(
        text.contains("Title: purlis panicked at crates/purlis-core/src/engine.rs:88:9"),
        "{text}"
    );
    assert!(text.contains("**purlis version:** 1.2.3"), "{text}");
    assert!(text.contains("> index out of bounds"), "{text}");
    assert!(
        !text.contains("somewhere"),
        "the backtrace stays home: {text}"
    );
}

#[test]
fn a_bug_with_no_description_and_no_panic_is_refused() {
    let w = World::new();
    let o = w.charter(&["report", "bug"], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("needs a description"), "{}", err(&o));
    assert!(w.calls().is_empty(), "{}", w.calls());
}
