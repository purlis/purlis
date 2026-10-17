// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge CLI rows: the CLI and its sign-in, asked only for a forge the project's repos use.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::doctor::Status;
use crate::forge::transport::{Call, NoAnswer, Reply};
use crate::forge::{Forge, Kind};

/// A transport whose sign-in answer is `signed_in`, counting how often it was asked.
struct SignIn {
    signed_in: bool,
    asked: AtomicUsize,
}

impl SignIn {
    fn new(signed_in: bool) -> Self {
        Self {
            signed_in,
            asked: AtomicUsize::new(0),
        }
    }
}

impl Transport for SignIn {
    fn send(&self, _forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
        panic!("the auth row sends no request");
    }

    fn check_auth(&self, forge: &Forge) -> Result<(), ForgeError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        if self.signed_in {
            Ok(())
        } else {
            Err(ForgeError::new(format!(
                "gh is not authenticated for {}. Run: gh auth login",
                forge.host
            )))
        }
    }
}

fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

#[test]
fn a_forge_no_repo_is_on_is_neither_looked_for_nor_asked() {
    let asked = SignIn::new(true);
    let cli = cli_row(
        &github(),
        Use::None,
        Some(std::path::Path::new("/usr/bin/gh")),
    );
    let auth = auth_row(&github(), Use::None, true, Asking::Ask(&asked));

    assert_eq!(cli.name, "gh");
    assert_eq!(cli.status, Status::Ok);
    assert_eq!(
        cli.detail,
        "not looked for: no workspace repo here has its origin on github.com. `purlis discover` \
         and `purlis clone` look for it when they run"
    );
    assert_eq!(auth.name, "gh auth");
    assert_eq!(auth.status, Status::Ok);
    assert_eq!(
        auth.detail,
        "not asked: no workspace repo here has its origin on github.com"
    );
    assert_eq!(asked.asked.load(Ordering::SeqCst), 0);
    assert!(!cli.deferred() && !auth.deferred());
}

#[test]
fn a_cli_a_repo_needs_is_named_where_it_was_found() {
    let row = cli_row(
        &github(),
        Use::Repos(2),
        Some(std::path::Path::new("/opt/homebrew/bin/gh")),
    );
    assert_eq!(row.status, Status::Ok);
    assert_eq!(
        row.detail,
        "/opt/homebrew/bin/gh (2 workspace repo(s) have their origin on github.com)"
    );
}

#[test]
fn a_cli_a_repo_needs_that_is_not_found_is_a_warning_and_its_sign_in_is_not_asked() {
    let asked = SignIn::new(true);
    let cli = cli_row(&github(), Use::Repos(1), None);
    let auth = auth_row(&github(), Use::Repos(1), false, Asking::Ask(&asked));

    assert_eq!(cli.status, Status::Warn);
    assert_eq!(
        cli.detail,
        "not found, and 1 workspace repo(s) have their origin on github.com"
    );
    assert!(
        cli.hint
            .starts_with("Install gh, then run `gh auth login`."),
        "{}",
        cli.hint
    );
    assert_eq!(auth.status, Status::Warn);
    assert_eq!(
        auth.detail,
        "not checked (gh is not found; the `gh` row says so)"
    );
    assert_eq!(asked.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn the_sign_in_is_the_clis_own_answer() {
    for (signed_in, status, detail) in [
        (true, Status::Ok, "signed in on github.com"),
        (
            false,
            Status::Warn,
            "gh is not authenticated for github.com. Run: gh auth login",
        ),
    ] {
        let asked = SignIn::new(signed_in);
        let row = auth_row(&github(), Use::Repos(1), true, Asking::Ask(&asked));
        assert_eq!((row.status, row.detail.as_str()), (status, detail));
        assert_eq!(asked.asked.load(Ordering::SeqCst), 1);
        if !signed_in {
            assert!(row.hint.contains("never reads the token"), "{}", row.hint);
        }
    }
}

#[test]
fn a_workspace_that_could_not_be_read_leaves_both_rows_not_checked_and_starts_no_cli() {
    let asked = SignIn::new(true);
    let why = "not checked (a workspace whose repos could not be read may have one on \
               github.com; the `workspace clones` row names it)";
    let auth = auth_row(&github(), Use::Unknown, true, Asking::Ask(&asked));
    let cli = cli_row(
        &github(),
        Use::Unknown,
        Some(std::path::Path::new("/usr/bin/gh")),
    );
    for row in [&auth, &cli] {
        assert_eq!(
            (row.status, row.detail.as_str()),
            (Status::Warn, why),
            "{row:?}"
        );
    }
    assert_eq!(asked.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn the_preflight_and_a_doctor_that_asks_no_forge_start_no_cli() {
    for (how, why) in [
        (
            Asking::Preflight,
            "the preflight a chat's start runs asks no forge CLI; `purlis doctor` asks gh",
        ),
        (Asking::NoForge, "this doctor asks no forge"),
    ] {
        let row = auth_row(&github(), Use::Repos(1), true, how);
        assert_eq!(row.status, Status::Warn);
        assert_eq!(row.detail, format!("not checked ({why})"));
        assert!(!row.deferred());
    }
}

#[test]
fn a_project_whose_repos_are_on_no_forge_gets_rows_that_look_for_nothing() {
    let (_d, root) = crate::doctor::tests::plane("schema = 1\n[[forge]]\nkind = \"github\"\n");
    let rows = rows(&crate::doctor::tests::doctor(&root));
    let said: Vec<(&str, Status)> = rows.iter().map(|r| (r.name.as_str(), r.status)).collect();
    assert_eq!(said, [("gh", Status::Ok), ("gh auth", Status::Ok)]);
    assert!(rows[0].detail.starts_with("not looked for: "), "{rows:?}");
}
