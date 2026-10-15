// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's question about a clone's remote (#1848), read against a config home with a
//! signed-in account and a clone given its remote, so no git runs.

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::forge::Kind;
use crate::forge::accounts::{Pasted, sign_in_with_token};
use crate::forge::recorded::Recorded;
use crate::forge::transport::Transport;
use crate::forge::{ForgeError, http::TokenSource};

/// A config home with one GitHub account signed in on `host`.
fn signed_in_on_host(host: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/accounts/github/token_check.json");
    let recorded = Arc::new(Recorded::parse(&std::fs::read_to_string(recording).unwrap()).unwrap());
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    sign_in_with_token(
        &HostScope::for_a_test(),
        home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        host,
        Pasted::new("kept-for-the-window-git-only".to_string()),
    )
    .unwrap();
    home
}

fn github(host: &str) -> Forge {
    Forge {
        kind: Kind::GitHub,
        host: host.to_string(),
    }
}

const WIDGET: &str = "github.example.com/acme/widget";

#[test]
fn an_unrecorded_clone_on_a_machine_with_no_forge_cli_asks_to_be_saved_once_naming_its_remote() {
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();

    let asked = question_with(
        home.path(),
        &github("github.example.com"),
        clone.path(),
        &[WIDGET.to_string()],
        &|_| false,
    );

    assert_eq!(
        asked,
        Some(Question::NotSavedYet {
            now: WIDGET.to_string()
        })
    );
}

#[test]
fn nothing_is_asked_where_automatic_saves_already_push() {
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let ask = |has_cli: bool, host: &str| {
        question_with(
            home.path(),
            &github(host),
            clone.path(),
            &[WIDGET.to_string()],
            &|_| has_cli,
        )
    };

    assert_eq!(
        ask(true, "github.example.com"),
        None,
        "the forge CLI pushes"
    );
    assert_eq!(
        ask(false, "gitlab.example.com"),
        None,
        "no purlis account on the host: nothing purlis's sign-in would do"
    );
    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();
    assert_eq!(ask(false, "github.example.com"), None, "recorded");
}

#[test]
fn the_persons_press_records_the_remote_so_the_next_automatic_save_signs_in() {
    use super::super::{HELPER_ARG, as_the_window, helper_for_push_with};
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let push = || {
        as_the_window(
            &HostScope::for_a_test(),
            Path::new("/Applications/purlis.app/purlis"),
            home.path(),
            || {
                helper_for_push_with(
                    &github("github.example.com"),
                    clone.path(),
                    "https://github.example.com/acme/widget.git",
                    &|_| false,
                    None,
                )
            },
        )
    };
    assert_eq!(push(), Err(super::super::NOT_SAVED_FROM_THE_WINDOW));

    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();

    assert!(push().is_ok_and(|helper| helper.contains(HELPER_ARG)));
}

#[test]
fn a_clone_whose_remote_changed_asks_naming_both_and_each_answer_is_remembered() {
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();
    let ask = |now: &str| {
        question_with(
            home.path(),
            &github("github.example.com"),
            clone.path(),
            &[now.to_string()],
            &|_| true,
        )
    };
    let moved = "github.example.com/someone/public";

    assert_eq!(
        ask(moved),
        Some(Question::Changed {
            recorded: WIDGET.to_string(),
            now: moved.to_string()
        }),
        "with a forge CLI too: the record holds the push"
    );
    assert_eq!(ask(WIDGET), None, "the remote it was saved to asks nothing");

    // "Save to B and remember it": the next save does not ask again.
    record(home.path(), clone.path(), moved.to_string()).unwrap();
    assert_eq!(ask(moved), None);
}

#[test]
fn keeping_the_recorded_remote_points_origin_back_at_it_with_no_push_url_of_its_own() {
    use crate::testgit::run;
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    run(clone.path(), &["init", "-q", "."]);
    run(
        clone.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.example.com/someone/public.git",
        ],
    );
    run(
        clone.path(),
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.example.com/someone/else.git",
        ],
    );
    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();

    let kept = keep(&HostScope::for_a_test(), home.path(), clone.path());

    assert_eq!(kept.as_deref(), Ok(WIDGET));
    let urls = run(
        clone.path(),
        &["remote", "get-url", "--all", "--push", "origin"],
    );
    assert_eq!(urls.out.trim(), format!("https://{WIDGET}"));
    assert_eq!(
        question(
            &HostScope::for_a_test(),
            home.path(),
            &github("github.example.com"),
            clone.path()
        ),
        None,
        "the next save asks nothing"
    );
}

#[test]
fn a_push_url_that_moved_alone_is_a_changed_remote_naming_it() {
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();
    let elsewhere = "github.example.com/someone/public";

    let asked = question_with(
        home.path(),
        &github("github.example.com"),
        clone.path(),
        &[WIDGET.to_string(), elsewhere.to_string()],
        &|_| true,
    );

    assert_eq!(
        asked,
        Some(Question::Changed {
            recorded: WIDGET.to_string(),
            now: elsewhere.to_string()
        })
    );
}

#[test]
fn a_press_records_only_the_remote_the_window_showed() {
    let home = signed_in_on_host("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let moved = "github.example.com/someone/public";

    let pressed = confirm_seen(home.path(), clone.path(), WIDGET, Some(moved.to_string()));

    assert!(pressed.is_err(), "{pressed:?}");
    assert!(
        super::super::read_push_remotes(home.path()).is_empty(),
        "nothing is recorded"
    );
    assert!(confirm_seen(home.path(), clone.path(), WIDGET, None).is_err());
    assert_eq!(
        confirm_seen(home.path(), clone.path(), WIDGET, Some(WIDGET.to_string())).as_deref(),
        Ok(WIDGET)
    );
}

/// #1985: the remote a repo's saves push to follows the repo to where the forge said it moved,
/// and a move records nothing that was not recorded.
#[test]
fn a_record_of_the_old_address_follows_the_repo_to_its_new_one_and_no_other_is_made() {
    let home = tempfile::tempdir().unwrap();
    let clone = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    record(home.path(), clone.path(), WIDGET.to_string()).unwrap();
    let key = |at: &Path| crate::sandbox::local::clone_key(at);
    let now = "github.example.com/purlis/widget";

    moved(
        &HostScope::for_a_test(),
        home.path(),
        clone.path(),
        WIDGET,
        now,
    );
    moved(
        &HostScope::for_a_test(),
        home.path(),
        elsewhere.path(),
        WIDGET,
        now,
    );
    // A record of another address is left as it is.
    moved(
        &HostScope::for_a_test(),
        home.path(),
        clone.path(),
        WIDGET,
        "github.example.com/x/y",
    );

    let recorded = super::super::read_push_remotes(home.path());
    assert_eq!(
        recorded.get(&key(clone.path())).map(String::as_str),
        Some(now)
    );
    assert_eq!(recorded.get(&key(elsewhere.path())), None);
}
