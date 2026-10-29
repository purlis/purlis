// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat started on a profile of this machine's starts as the profile's sign-in says (#2036,
//! ADR 0087 as amended): its run's model source is the sign-in's kind, and what it is billed to
//! is the profile's label. An `api-key` or `cloud` profile names its credentials by vault
//! reference, and the start carries those references and never a value. A key waits for the
//! key proxy (#822), and this version hands no cloud's credentials (#2151): such a chat starts
//! nothing, and says why.
//!
//! The profile store is found through this process's config home and data home, so each body
//! runs in a child whose homes are the test's own folder, with a stand-in harness first on its
//! `PATH`, as `a_chat_on_a_profile_made_here_runs_in_its_folder` does.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness_declaration::SignInKind;
use purlis_core::profilesignin::SignIn;
use purlis_core::profilestore::{self, Homes, MachineSettings};
use purlis_core::start::{self, Start};

/// Set on the child: the data home it was started with.
const DATA: &str = "PROFILE_SIGN_IN_DATA_HOME";

fn in_a_data_home_of_its_own(test: &str) -> Option<PathBuf> {
    if let Some(data) = std::env::var_os(DATA) {
        return Some(PathBuf::from(data));
    }
    let data = tempfile::tempdir().unwrap();
    let programs = tempfile::tempdir().unwrap();
    // It answers its version, and its sign-in status as the test left it ([`signed_out`]).
    stand_in::program(
        programs.path(),
        "claude",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = auth ]; then\n  [ -f '{}' ] && exit 1\n  exit 0\nfi\necho \
             '2.1.288 (Claude Code)'\n",
            data.path().join(SIGNED_OUT).display()
        ),
    );
    let path = std::env::join_paths(std::iter::once(programs.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    purlis_core::testrun::rerun_steered(
        &[test],
        &[
            (purlis_core::datahome::HOME_VAR, data.path().as_os_str()),
            (purlis_core::machine::HOME_VAR, data.path().as_os_str()),
            (DATA, data.path().as_os_str()),
            ("PATH", path.as_os_str()),
        ],
    );
    None
}

/// The file whose presence makes the stand-in harness say it is not signed in.
const SIGNED_OUT: &str = "signed-out";

/// From now on, the stand-in harness says this machine's profile is not signed in.
fn signed_out(data: &Path) {
    fs::write(data.join(SIGNED_OUT), "").unwrap();
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

fn start_on(name: &str, root: &Path) -> Start {
    Start {
        profile: Some(name.to_owned()),
        persona: None,
        name: "ide.7".to_owned(),
        cwd: Some(root.to_path_buf()),
        resume: None,
        show_footer: false,
        resuming: None,
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        made: Default::default(),
    }
}

/// Makes the Claude Code profile `work` and gives it `sign_in`.
fn work_signed_in(sign_in: SignIn) {
    let store = Homes::here().expect("the test's data home");
    let machine = tempfile::tempdir().unwrap();
    let settings = machine.path().join("claude-settings");
    fs::create_dir_all(&settings).unwrap();
    let named = settings.display().to_string();
    let env = move |asked: &str| (asked == "CLAUDE_CONFIG_DIR").then(|| named.clone());
    profilestore::create(
        &store,
        "work",
        "claude",
        &MachineSettings {
            home: machine.path(),
            env: &env,
        },
        1,
    )
    .expect("the profile is made");
    profilestore::sign_in_with(store.config_root(), "work", Some(sign_in))
        .expect("the sign-in is kept");
}

#[test]
fn a_chat_on_a_subscription_profile_starts_with_its_kind_and_label_and_no_credential() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_chat_on_a_subscription_profile_starts_with_its_kind_and_label_and_no_credential",
    ) else {
        return;
    };
    work_signed_in(SignIn {
        kind: "subscription".to_owned(),
        label: Some("Work plan".to_owned()),
        credentials: Default::default(),
        rest: Default::default(),
    });
    let plane = project();

    let ready = start::ready(&start_on("work", plane.path()), plane.path()).expect("it starts");

    let sign_in = ready.sign_in.expect("the start says how it signs in");
    assert_eq!(sign_in.kind, SignInKind::Subscription);
    assert_eq!(sign_in.billed_to, "your plan · Work plan");
    assert!(sign_in.credentials.is_empty());
}

#[test]
fn a_chat_on_a_key_profile_waits_for_the_key_proxy_and_starts_nothing() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_chat_on_a_key_profile_waits_for_the_key_proxy_and_starts_nothing",
    ) else {
        return;
    };
    work_signed_in(SignIn {
        kind: "api-key".to_owned(),
        label: Some("Work Anthropic key".to_owned()),
        credentials: [(
            "ANTHROPIC_API_KEY".to_owned(),
            "vault:providers/anthropic".to_owned(),
        )]
        .into(),
        rest: Default::default(),
    });
    let plane = project();

    let why = start::ready(&start_on("work", plane.path()), plane.path()).unwrap_err();

    assert!(why.contains("proxy"), "{why}");
    assert!(why.contains("Nothing was started"), "{why}");
}

#[test]
fn a_sign_in_its_harness_cannot_take_is_refused_when_it_is_kept() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_sign_in_its_harness_cannot_take_is_refused_when_it_is_kept",
    ) else {
        return;
    };
    work_signed_in(SignIn {
        kind: "subscription".to_owned(),
        label: None,
        credentials: Default::default(),
        rest: Default::default(),
    });
    let store = Homes::here().unwrap();

    let refused = profilestore::sign_in_with(
        store.config_root(),
        "work",
        Some(SignIn {
            kind: "local".to_owned(),
            label: None,
            credentials: Default::default(),
            rest: Default::default(),
        }),
    );

    assert!(refused.is_err());
    let kept = profilestore::list(store.config_root()).unwrap();
    assert_eq!(
        kept[0].sign_in.as_ref().map(|s| s.kind.as_str()),
        Some("subscription"),
        "a refused sign-in leaves the one it had"
    );
}

fn bedrock() -> SignIn {
    SignIn {
        kind: "cloud".to_owned(),
        label: Some("team account".to_owned()),
        credentials: [
            ("CLAUDE_CODE_USE_BEDROCK", "vault:aws/use"),
            ("AWS_REGION", "vault:aws/region"),
            ("AWS_ACCESS_KEY_ID", "vault:aws/id"),
            ("AWS_SECRET_ACCESS_KEY", "vault:aws/secret"),
        ]
        .map(|(v, r)| (v.to_owned(), r.to_owned()))
        .into(),
        rest: Default::default(),
    }
}

#[test]
fn a_chat_on_a_cloud_profile_says_its_credentials_are_not_handed_and_starts_nothing() {
    purlis_core::unsteered!();
    let Some(_data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_chat_on_a_cloud_profile_says_its_credentials_are_not_handed_and_starts_nothing",
    ) else {
        return;
    };
    work_signed_in(bedrock());
    let plane = project();

    let why = start::ready(&start_on("work", plane.path()), plane.path()).unwrap_err();

    assert!(why.contains("cloud credentials"), "{why}");
    assert!(why.contains("Nothing was started"), "{why}");
}

#[test]
fn a_cloud_profile_whose_harness_is_signed_out_is_never_asked_to_sign_in() {
    purlis_core::unsteered!();
    // A cloud signs the chat in, not the harness's own plan (#2148's check is for that).
    let Some(data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_cloud_profile_whose_harness_is_signed_out_is_never_asked_to_sign_in",
    ) else {
        return;
    };
    signed_out(&data);
    work_signed_in(bedrock());
    let plane = project();

    let why = start::ready(&start_on("work", plane.path()), plane.path()).unwrap_err();

    assert!(why.contains("cloud credentials"), "{why}");
    assert!(!why.contains("not signed in"), "{why}");
}

#[test]
fn a_kept_subscription_profile_that_is_signed_out_is_still_refused_with_sign_in() {
    purlis_core::unsteered!();
    let Some(data) = in_a_data_home_of_its_own(
        "a_chat_on_a_profile_starts_as_its_sign_in_says::a_kept_subscription_profile_that_is_signed_out_is_still_refused_with_sign_in",
    ) else {
        return;
    };
    signed_out(&data);
    work_signed_in(SignIn {
        kind: "subscription".to_owned(),
        label: Some("Work plan".to_owned()),
        credentials: Default::default(),
        rest: Default::default(),
    });
    let plane = project();

    let why = start::ready(&start_on("work", plane.path()), plane.path()).unwrap_err();

    assert!(why.contains("not signed in"), "{why}");
    assert!(why.contains("Sign in"), "{why}");
}
