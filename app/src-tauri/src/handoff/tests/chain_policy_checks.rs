// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A dispatched chat's vault checks in the window run under its chain's `[policy]`** (#7153,
//! #7164): where this project's own `vaults` is `off` and the chain that dispatched the chat ran
//! it `enforce`d, each check answers what the chat's brokered read does. Each test is red where
//! its call site builds a bare `Ctx`.

use super::*;

/// A chain that ran the `vaults` family enforced, as the app spells it.
const STRICT_CHAIN: &str = "vaults=enforce";

/// How every family runs where the project's `[policy]` turns `vaults` off, for this test's
/// thread alone; put away as it goes.
struct VaultsOff;

fn vaults_off_here() -> VaultsOff {
    let resolved = purlis_core::enforcement::Resolved::of(
        &purlis_core::enforcement::Settings::from_text(Some("[policy]\nvaults = \"off\"\n")),
        &purlis_core::sandbox::policy::Locks::none(),
    );
    purlis_core::enforcement::answered_on_this_thread(Some(resolved));
    VaultsOff
}

impl Drop for VaultsOff {
    fn drop(&mut self) {
        purlis_core::enforcement::answered_on_this_thread(None);
    }
}

/// A chat as `ops` that a dispatch with `policy` started.
fn a_dispatched_chat(held: &Held, plane: &Plane, policy: Option<&str>) -> u32 {
    // A name of its own per policy, so two chats in one plane never share one.
    let name = if policy.is_some() { "1" } else { "2" };
    let start = purlis_core::start::Start {
        profile: Some("work".to_owned()),
        persona: Some("ops".to_owned()),
        name: name.to_owned(),
        cwd: Some(plane.root.clone()),
        made: if crate::sandboxing::claude_is_wrapped_here_for_tests() {
            purlis_core::sandbox::Made::Window
        } else {
            purlis_core::sandbox::Made::default()
        },
        ..Default::default()
    };
    let ready = purlis_core::start::ready(&start, &plane.root).expect("the chat starts");
    let chat = Chat {
        program: ready.program.clone(),
        cwd: ready.cwd.clone(),
        name: name.to_owned(),
        resume: ready.session.clone(),
        profile: Some("work".to_owned()),
        persona: Some("ops".to_owned()),
        from: Some(HandedFrom {
            id: None,
            chat: 1,
            name: "steward 1".to_owned(),
            workspace: Place::PlaneRoot,
            report: Owed::Due,
            mode: Mode::Task,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
            policy: policy.map(str::to_owned),
        }),
        ..Default::default()
    };
    held.chats()
        .start_ready(&chat, &ready, STARTING)
        .expect("it runs")
}

fn the_plane() -> (Plane, tempfile::TempDir) {
    let plane = Plane::new().a_persona("ops", "");
    let extra = tempfile::tempdir().expect("a folder");
    std::fs::write(
        plane.root.join("vaults.json"),
        r#"{"vaults": {"team": {"provider": "plain-file", "config": {"file": "t.json"}, "persona": "ops"}, "keep": {"provider": "plain-file", "config": {"file": "k.json"}, "persona": "ops"}, "finance": {"provider": "plain-file", "config": {"file": "f.json"}, "persona": "billing"}}}"#,
    )
    .expect("a registry");
    std::fs::write(plane.root.join("k.json"), r#"{"SHARED": "sh4red-v4lue"}"#).expect("keep");
    std::fs::write(plane.root.join("f.json"), r#"{"STRIPE": "str1pe-v4lue"}"#).expect("finance");
    (plane, extra)
}

#[test]
fn a_pool_runs_vault_check_runs_under_its_chats_chain_policy() {
    // pooltools: `desktop_ctx` is what `run_on_its_runner` judges each run with.
    let (plane, _extra) = the_plane();
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chained = a_dispatched_chat(&held, &plane, Some(STRICT_CHAIN));
    let plain = a_dispatched_chat(&held, &plane, None);
    let _off = vaults_off_here();
    let may = |chat: u32| {
        let ctx = crate::pooltools::desktop_ctx(&held, chat);
        purlis_core::secrets::brokered::authorise(&ctx, Some("ops"), "finance").is_ok()
    };
    assert!(may(plain), "vaults off here: ops may use it");
    assert!(!may(chained), "its chain enforced vaults");
}

#[test]
fn a_replace_ask_into_a_vault_the_persona_may_not_use_is_refused_under_its_chats_chain_policy() {
    let (plane, _extra) = the_plane();
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_dispatched_chat(&held, &plane, Some(STRICT_CHAIN));
    let line = purlis_core::secrets::ask::SecretAsk {
        chat,
        key: "STRIPE".to_owned(),
        purpose: "the API".to_owned(),
        vault: Some("finance".to_owned()),
        replace: Some("it was rotated".to_owned()),
        program: None,
    };
    std::thread::scope(|scope| {
        let asking = scope.spawn(|| {
            // The override is the asking thread's own, and `asked` runs on this one.
            let _off = vaults_off_here();
            crate::secretasks::asked(&held, &line)
        });
        // Where nothing refuses it, the person is asked: decline, so nothing waits, and fail.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !asking.is_finished() {
            let ctx = purlis_core::secrets::Ctx::new(
                &plane.root,
                purlis_core::secrets::Env::from_process(),
            );
            for open in held.secret_asks().open() {
                let _ = held.secret_asks().answer(
                    &ctx,
                    open.id,
                    purlis_core::secrets::ask::Answer::Decline,
                    crate::asking::the_window(),
                );
            }
            assert!(std::time::Instant::now() < deadline, "the ask hung");
            std::thread::yield_now();
        }
        let said = asking.join().expect("answered");
        assert!(
            matches!(&said, purlis_core::hookwire::Answer::No { why }
                if why.contains("A replace is asked only in a vault ops may use")),
            "{said:?}"
        );
    });
}

#[test]
fn a_run_notice_link_into_a_vault_the_persona_may_not_use_is_refused_under_its_chats_chain_policy()
{
    let (plane, _extra) = the_plane();
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_dispatched_chat(&held, &plane, Some(STRICT_CHAIN));
    let _off = vaults_off_here();
    let mut need = curl_lacking_token(&plane);
    need.bound.chat = chat;
    let ran = held_while(&held, need, |open| {
        let pressed = crate::secretasks::run_grant_given(
            &held,
            None,
            open.id,
            crate::secretasks::RunGrantPress::Refuse,
            picked_in("TOKEN", "finance", "STRIPE"),
        );
        assert!(
            pressed
                .as_ref()
                .is_err_and(|why| why.contains("can't use vault finance")),
            "{pressed:?}"
        );
        // Put the notice away, so the run ends.
        let _ = crate::secretasks::run_grant_given(
            &held,
            None,
            open.id,
            crate::secretasks::RunGrantPress::Refuse,
            crate::secretasks::RunGiven::default(),
        );
    });
    assert!(matches!(
        ran,
        purlis_core::secrets::ask::run::RunHeld::Refused { .. }
    ));
}

#[test]
fn keep_blocked_on_a_run_notice_for_a_vault_the_persona_may_not_use_holds_under_its_chats_chain_policy()
 {
    let (plane, _extra) = the_plane();
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_dispatched_chat(&held, &plane, Some(STRICT_CHAIN));
    let _off = vaults_off_here();
    let mut need = curl_on_team_not_its(
        &plane,
        purlis_core::secrets::ask::run::Offer::Vault { no_allow: None },
        &["TOKEN"],
    );
    need.bound.chat = chat;
    need.bound.vault = "finance".to_owned();
    let asked = raised_and_left(&held, need);
    let pressed = crate::secretasks::run_grant_given(
        &held,
        None,
        asked,
        crate::secretasks::RunGrantPress::Refuse,
        crate::secretasks::RunGiven::default(),
    )
    .expect("kept");
    assert!(
        pressed.is_none(),
        "ops may not use finance under its chain, so Keep blocked holds: {pressed:?}"
    );
}
