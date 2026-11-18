// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A dispatch from an unattended chat (#1446): which grants count for it, what a missing one
//! is answered with, and what the chat it starts takes from it. Every expected answer and
//! sentence is written out.

use std::path::Path;

use super::*;
use crate::dispatchgrant::{ChatPair, Pair};

fn locks(json: &str) -> Locks {
    Locks::parse(json, Path::new("/etc/purlis/policy.json"))
}

fn pair(asking: &str, target: &str) -> Pair {
    Pair::new(asking, target).expect("a pair")
}

fn for_this_chat(asking: Option<&str>, target: &str) -> InForce {
    InForce {
        chat: vec![ChatPair {
            asking: asking.map(str::to_owned),
            target: target.to_owned(),
        }],
        ..InForce::default()
    }
}

fn missing(asking: Option<&str>, target: &str) -> Answer {
    Answer::Refused(Refusal::Missing(Missing {
        asking: asking.map(str::to_owned),
        target: target.to_owned(),
        unreviewed: false,
    }))
}

/// [`super::covers`] for a chat the app started inside a sandbox, in a project whose file
/// names no pair this machine has not reviewed.
fn covers(asking: Option<&str>, target: &str, grants: &InForce, policy: &Locks) -> Answer {
    super::covers(asking, target, grants, policy, false, true)
}

// ---- which grants count -------------------------------------------------------------------------

#[test]
fn a_grant_for_the_person_on_this_machine_or_for_the_project_covers_an_unattended_chat() {
    for grants in [
        InForce {
            you: vec![pair("steward", "devops")],
            ..InForce::default()
        },
        InForce {
            project: vec![pair("steward", "devops")],
            ..InForce::default()
        },
    ] {
        assert_eq!(
            covers(Some("steward"), "devops", &grants, &Locks::none()),
            Answer::Covered,
            "{grants:?}"
        );
        // And no other pair: not the way back, not another target, not another asker.
        for (asking, target) in [("devops", "steward"), ("steward", "qa"), ("qa", "devops")] {
            assert_eq!(
                covers(Some(asking), target, &grants, &Locks::none()),
                missing(Some(asking), target),
                "{asking} to {target} under {grants:?}"
            );
        }
    }
}

#[test]
fn a_grant_made_for_one_chat_never_counts_for_an_unattended_chat() {
    let grants = for_this_chat(Some("steward"), "devops");
    // It covers the chat while a person answers it.
    assert_eq!(
        dispatchgrant::covers(Some("steward"), "devops", &grants, &Locks::none()),
        Covers::Covered
    );

    assert_eq!(
        covers(Some("steward"), "devops", &grants, &Locks::none()),
        missing(Some("steward"), "devops")
    );
    // A chat on no persona is covered by nothing else, so by nothing.
    assert_eq!(
        covers(
            None,
            "devops",
            &for_this_chat(None, "devops"),
            &Locks::none()
        ),
        missing(None, "devops")
    );
}

#[test]
fn nothing_in_force_is_a_refusal_and_never_a_question_for_the_person() {
    let answer = covers(
        Some("steward"),
        "devops",
        &InForce::default(),
        &Locks::none(),
    );

    assert_eq!(answer, missing(Some("steward"), "devops"));
    // The attended answer to the same ask is the one that raises a Notice.
    assert_eq!(
        dispatchgrant::covers(
            Some("steward"),
            "devops",
            &InForce::default(),
            &Locks::none()
        ),
        Covers::NeedsGrant
    );
}

#[test]
fn its_own_persona_needs_no_grant_as_for_any_chat() {
    assert_eq!(
        covers(
            Some("devops"),
            "devops",
            &InForce::default(),
            &Locks::none()
        ),
        Answer::Covered
    );
}

#[test]
fn a_policy_lock_refuses_an_unattended_chat_as_it_does_any_other() {
    let pair_locked =
        locks(r#"{"owner": "IT", "dispatch": {"locked": [{"from": "steward", "to": "devops"}]}}"#);
    let granted = InForce {
        you: vec![pair("steward", "devops")],
        project: vec![pair("steward", "devops")],
        ..InForce::default()
    };
    assert_eq!(
        covers(Some("steward"), "devops", &granted, &pair_locked),
        Answer::Locked(
            "Policy forbids steward chats dispatching to devops. Locked by policy, set by IT in \
             /etc/purlis/policy.json."
                .to_owned()
        )
    );
    // All dispatch locked holds a chat's own persona too.
    let all = locks(r#"{"owner": "IT", "dispatch": {"allow": false}}"#);
    assert_eq!(
        covers(Some("devops"), "devops", &granted, &all),
        Answer::Locked(
            "Policy forbids one chat dispatching to another. Locked by policy, set by IT in \
             /etc/purlis/policy.json."
                .to_owned()
        )
    );
}

// ---- the sentences ------------------------------------------------------------------------------

#[test]
fn the_refusal_names_the_missing_pair_and_where_a_person_grants_it() {
    let Answer::Refused(why) = covers(
        Some("steward"),
        "devops",
        &InForce::default(),
        &Locks::none(),
    ) else {
        panic!("refused");
    };
    assert_eq!(
        why.say(),
        "this chat runs with its harness's permission prompts off, so nobody is here to answer \
         for a dispatch, and no grant lets steward chats dispatch to devops: none for the \
         person on this machine, and none for this project. A grant made for one chat does not \
         count here. Only a person makes one: they grant it under Settings › Project › \
         Dispatch, for themselves on this machine or for everyone in this project. Until then, \
         do this work without devops, or say in what you leave behind that it is waiting."
    );
}

#[test]
fn whether_the_project_names_the_pair_is_the_file_s_word_and_not_what_is_in_force() {
    // #1586: a pair the file names that nobody here reviewed is not in force here, so what is
    // in force cannot say the file names it. The caller asks the file, as the app does.
    let unreviewed = super::covers(
        Some("steward"),
        "devops",
        &InForce::default(),
        &Locks::none(),
        true,
        true,
    );
    assert!(
        matches!(&unreviewed, Answer::Refused(Refusal::Missing(missing)) if missing.unreviewed),
        "{unreviewed:?}"
    );
    // And a project pair in force here is covered whatever the caller says of the file.
    let accepted = InForce {
        project: vec![pair("steward", "devops")],
        ..InForce::default()
    };
    for named in [true, false] {
        assert_eq!(
            super::covers(
                Some("steward"),
                "devops",
                &accepted,
                &Locks::none(),
                named,
                true
            ),
            Answer::Covered
        );
    }
}

#[test]
fn a_project_pair_nobody_reviewed_on_this_machine_is_refused_and_says_to_review_it() {
    // D-1437-R1: the grant answers "needs a grant" for a pair the project's file names and
    // this machine has not acknowledged. For an unattended chat that is a refusal.
    let answer = answer_of(Covers::NeedsGrant, Some("steward"), "devops", true, true);

    let Answer::Refused(why) = answer else {
        panic!("refused");
    };
    assert!(matches!(&why, Refusal::Missing(missing) if missing.unreviewed));
    assert_eq!(
        why.say(),
        "this chat runs with its harness's permission prompts off, so nobody is here to answer \
         for a dispatch. The project's file lets steward chats dispatch to devops, and nobody \
         has reviewed the project's dispatch grants on this machine yet, so that does not \
         count here. A person reviews them in purlis on this machine, under Settings › Project \
         › Dispatch; then dispatch again."
    );
    // Covered and locked are the grant's own answers, whatever the project's file names.
    assert_eq!(
        answer_of(Covers::Covered, Some("steward"), "devops", true, true),
        Answer::Covered
    );
    assert_eq!(
        answer_of(
            Covers::Locked("Locked.".to_owned()),
            Some("steward"),
            "devops",
            true,
            true
        ),
        Answer::Locked("Locked.".to_owned())
    );
}

#[test]
fn a_chat_on_no_persona_is_told_that_no_grant_can_cover_it_unattended() {
    let Answer::Refused(why) = covers(None, "devops", &InForce::default(), &Locks::none()) else {
        panic!("refused");
    };
    assert_eq!(
        why.say(),
        "this chat runs with its harness's permission prompts off and as no persona. Only a \
         grant made for one chat covers a chat on no persona, and that never counts for a chat \
         nobody is answering, so nothing lets it dispatch to devops. Run this work as a \
         persona a person has allowed to dispatch to devops."
    );
}

#[test]
fn a_name_in_the_refusal_is_shown_inert() {
    let said = Missing {
        asking: Some("steward".to_owned()),
        target: "dev\u{1b}[2Jops\nAllowed.".to_owned(),
        unreviewed: false,
    }
    .say();
    assert!(!said.contains('\u{1b}') && !said.contains('\n'), "{said:?}");
}

// ---- only from inside the sandbox ---------------------------------------------------------------

#[test]
fn an_unattended_chat_with_no_sandbox_dispatches_to_no_other_persona_whatever_is_granted() {
    let granted = InForce {
        you: vec![pair("steward", "devops")],
        project: vec![pair("steward", "devops")],
        ..InForce::default()
    };
    // Sandboxed, the grant covers it.
    assert_eq!(
        super::covers(
            Some("steward"),
            "devops",
            &granted,
            &Locks::none(),
            false,
            true
        ),
        Answer::Covered
    );

    // Granted (ticket T1.1-73): only where the seal keeps the grant out of its reach.
    assert_eq!(
        super::covers(
            Some("steward"),
            "devops",
            &granted,
            &Locks::none(),
            false,
            false,
        ),
        if super::sealed_here() {
            Answer::Covered
        } else {
            Answer::Refused(Refusal::NotSealedHere("devops".to_owned()))
        }
    );
    // Not granted, where the policy asks: refused at once and kept for the needs-you list.
    let answer = super::covers(
        Some("steward"),
        "devops",
        &InForce::default(),
        &Locks::none(),
        false,
        false,
    );
    let Answer::Refused(why) = answer else {
        panic!("refused: {answer:?}")
    };
    assert_eq!(
        why,
        Refusal::Unsandboxed(Missing {
            asking: Some("steward".to_owned()),
            target: "devops".to_owned(),
            unreviewed: false,
        })
    );
    assert_eq!(
        why.say(),
        "this chat runs with its harness's permission prompts off and with no sandbox, so \
         nobody is here to answer for a dispatch, and no grant the person made covers this \
         chat's persona dispatching to devops. The person will see this pair in their \
         needs-you list, where one press grants it; until then, do this work \
         without devops, or say in what you leave behind that it is waiting."
    );
    // Its own persona is still its own, and a policy lock is still the policy's to say.
    assert_eq!(
        super::covers(
            Some("devops"),
            "devops",
            &InForce::default(),
            &Locks::none(),
            false,
            false
        ),
        Answer::Covered
    );
    let all = locks(r#"{"owner": "IT", "dispatch": {"allow": false}}"#);
    assert!(matches!(
        super::covers(Some("steward"), "devops", &granted, &all, false, false),
        Answer::Locked(_)
    ));
    // A chat on no persona has no persona of its own to dispatch to.
    assert_eq!(
        super::covers(
            None,
            "devops",
            &InForce::default(),
            &Locks::none(),
            false,
            false
        ),
        Answer::Refused(Refusal::Unsandboxed(Missing {
            asking: None,
            target: "devops".to_owned(),
            unreviewed: false,
        }))
    );
}

// ---- what it starts -----------------------------------------------------------------------------

fn words(command: &[&str]) -> Vec<String> {
    command.iter().map(|word| (*word).to_owned()).collect()
}

/// A start as a careless caller might build it from an unattended asking chat's record: with
/// everything of that chat's still on it.
fn the_asking_chats_own() -> Start {
    Start {
        profile: Some("work".to_owned()),
        persona: Some("steward".to_owned()),
        name: "check the queue".to_owned(),
        cwd: Some("/project/workspaces/alpha/svc".into()),
        resume: crate::harness::SessionId::new("0b0f2b7e-6f0f-4b6e-9d57-0d0c5a3c2b1a").ok(),
        show_footer: false,
        resuming: Some("workspaces/alpha/sessions/2026-10-07.md".to_owned()),
        without_sandbox: Some(crate::sandbox::OptOut {
            reason: Some("a quick look".to_owned()),
        }),
        held: Some(crate::reopen::HeldGrants {
            persona: Some("steward".to_owned()),
        }),
        grants: crate::sandbox::grant::Grants {
            hosts: Vec::new(),
            writes: vec!["/Users/o/elsewhere".into()],
            ..Default::default()
        },
        // The asking chat's own start was the person's.
        made: crate::sandbox::Made::Window,
    }
}

#[test]
fn the_persona_chat_starts_with_no_held_grants_no_opt_out_and_no_bypass() {
    let work = words(&["claude"]);

    let start = start_of_a_persona_chat(
        the_asking_chats_own(),
        "devops",
        Some(Inherited {
            profile: "work",
            command: &work,
        }),
    );

    assert_eq!(
        start,
        Ok(Start {
            profile: Some("work".to_owned()),
            persona: Some("devops".to_owned()),
            name: "check the queue".to_owned(),
            cwd: Some("/project/workspaces/alpha/svc".into()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: crate::sandbox::grant::Grants::default(),
            // Never the asking chat's own way of being started: a start that names no asker
            // never takes the person's `off` (#2071).
            made: crate::sandbox::Made::Unnamed,
        })
    );
}

#[test]
fn a_profile_that_switches_the_prompts_off_is_not_passed_on_to_the_persona_chat() {
    for (command, flag) in [
        (
            words(&["claude", "--dangerously-skip-permissions"]),
            "--dangerously-skip-permissions",
        ),
        (
            words(&["claude", "--permission-mode", "bypassPermissions"]),
            "--permission-mode bypassPermissions",
        ),
        (
            words(&["claude", "--permission-mode=bypassPermissions"]),
            "--permission-mode=bypassPermissions",
        ),
        (
            words(&["codex", "--dangerously-bypass-approvals-and-sandbox"]),
            "--dangerously-bypass-approvals-and-sandbox",
        ),
        (words(&["codex", "--yolo"]), "--yolo"),
    ] {
        assert_eq!(bypass_in(&command).as_deref(), Some(flag), "{command:?}");
        assert_eq!(
            start_of_a_persona_chat(
                the_asking_chats_own(),
                "devops",
                Some(Inherited {
                    profile: "night",
                    command: &command,
                }),
            ),
            Err(format!(
                "profile 'night' starts its harness with the permission prompts off ({flag}), \
                 and a task never takes that from the chat that dispatched it. \
                 Dispatch from a chat on a profile that asks."
            )),
            "{command:?}"
        );
    }
}

#[test]
fn a_profile_that_asks_is_passed_on_whatever_else_its_command_says() {
    for command in [
        words(&["claude"]),
        words(&["ccs", "work"]),
        words(&["claude", "--permission-mode", "default"]),
        words(&["claude", "--permission-mode=acceptEdits"]),
        words(&["claude", "--model", "bypassPermissions"]),
        words(&["claude", "--permission-mode"]),
        words(&["codex", "-a", "on-request"]),
    ] {
        assert_eq!(bypass_in(&command), None, "{command:?}");
    }
    // With no profile handed in there is nothing to refuse here: a caller that knows who
    // named the profile asks `bypass_refusal` of it first.
    assert!(start_of_a_persona_chat(the_asking_chats_own(), "devops", None).is_ok());
}

#[test]
fn codex_s_own_ways_of_asking_nobody_are_recognised_and_its_asking_ones_are_not() {
    for (command, flag) in [
        (words(&["codex", "--full-auto"]), Some("--full-auto")),
        (words(&["codex", "-a", "never"]), Some("-a never")),
        // A short flag with its value attached, as the command line itself reads one.
        (words(&["codex", "-anever"]), Some("-anever")),
        (
            words(&["codex", "-capproval_policy=never"]),
            Some("-capproval_policy=never"),
        ),
        (
            words(&["codex", "--ask-for-approval", "never"]),
            Some("--ask-for-approval never"),
        ),
        (
            words(&["codex", "--ask-for-approval=never"]),
            Some("--ask-for-approval=never"),
        ),
        // The same policy, set as a configuration override, quoted or not (#1509).
        (
            words(&["codex", "-c", "approval_policy=never"]),
            Some("-c approval_policy=never"),
        ),
        (
            words(&["codex", "--config", "approval_policy=\"never\""]),
            Some("--config approval_policy=\"never\""),
        ),
        (
            words(&["codex", "--config=approval_policy='never'"]),
            Some("--config=approval_policy='never'"),
        ),
        (
            words(&["codex", "-c", "approval_policy = \"never\""]),
            Some("-c approval_policy = \"never\""),
        ),
        // A first message that only talks about a flag is not one.
        (words(&["codex", "run it with -a never please"]), None),
        (words(&["codex", "-c", "approval_policy=on-request"]), None),
        (words(&["codex", "-c", "model=never"]), None),
        (words(&["codex", "-a", "on-request"]), None),
        (words(&["codex", "--ask-for-approval", "untrusted"]), None),
        // A value is the word after its flag, and nothing further along.
        (words(&["codex", "-a", "on-failure", "never"]), None),
    ] {
        assert_eq!(bypass_in(&command).as_deref(), flag, "{command:?}");
    }
}

#[test]
fn no_chat_is_started_for_another_on_a_profile_that_asks_nobody_whoever_named_it() {
    let yolo = words(&["claude", "--dangerously-skip-permissions"]);
    let asks = words(&["claude"]);
    for by in [
        NamedBy::TheDispatch,
        NamedBy::ThePersona("devops"),
        NamedBy::TheAskingChat,
    ] {
        assert_eq!(bypass_refusal("work", &asks, by), None, "{by:?}");
        let said = bypass_refusal("yolo", &yolo, by).expect("refused");
        assert!(said.contains("'yolo'"), "{said}");
        assert!(said.contains("(--dangerously-skip-permissions)"), "{said}");
        assert!(!said.contains('\n'), "{said}");
    }
    assert_eq!(
        bypass_refusal("yolo", &yolo, NamedBy::TheDispatch).as_deref(),
        Some(
            "the dispatch names profile 'yolo', which starts its harness with the permission \
             prompts off (--dangerously-skip-permissions), and purlis starts no chat for \
             another chat on such a profile. Name a profile that asks, or none."
        )
    );
    assert_eq!(
        bypass_refusal("yolo", &yolo, NamedBy::ThePersona("devops")).as_deref(),
        Some(
            "persona 'devops' names profile 'yolo', which starts its harness with the \
             permission prompts off (--dangerously-skip-permissions), and purlis starts no \
             chat for another chat on such a profile. Give the persona a profile that asks, \
             from its view, or name one with --profile."
        )
    );
}

/// **The `dispatch` family for a chat nobody is at** (Q308, instruction 50): under `off` or
/// `log` a pair nothing covers, and one refused only because the asking chat has no sandbox, is
/// covered; the person's nevers and a lock hold in every mode.
#[test]
fn an_unsandboxed_or_ungranted_dispatch_is_covered_only_where_the_policy_does_not_enforce_it() {
    use crate::enforcement::Mode;
    let unsandboxed = answer_of(Covers::Covered, Some("steward"), "devops", false, false);
    let missing = answer_of(Covers::NeedsGrant, Some("steward"), "devops", false, true);
    let never = answer_of(Covers::Never, Some("steward"), "devops", false, true);
    for refused in [&unsandboxed, &missing] {
        assert_eq!(
            as_the_policy_runs(refused.clone(), Mode::Enforce),
            refused.clone()
        );
        for mode in [Mode::Off, Mode::Log] {
            assert_eq!(as_the_policy_runs(refused.clone(), mode), Answer::Covered);
        }
    }
    for mode in [Mode::Off, Mode::Log, Mode::Enforce] {
        assert_eq!(as_the_policy_runs(never.clone(), mode), never.clone());
    }
}

/// A project at a fresh folder whose committed file holds `[policy]` `text`.
fn a_project_whose_policy_is(text: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a project");
    std::fs::write(
        crate::names::manifest(root.path()),
        format!("[policy]\n{text}"),
    )
    .expect("the manifest");
    root
}

#[test]
fn a_profile_with_its_prompts_off_is_refused_a_dispatched_chat_as_policy_mode_runs() {
    // Q308b: the refusal is `mode`'s. Enforced, it refuses as it always has; logged, the chat
    // starts and one line says it would have been refused; off, nothing is said.
    let yolo = words(&["claude", "--dangerously-skip-permissions"]);
    let chat = "01J9ZQ3V5N8X4T2K7M6P0R1S2C";
    for (text, refused, lines) in [
        ("template = \"strict\"\n", true, 0),
        ("template = \"strict\"\nmode = \"log\"\n", false, 1),
        ("template = \"team\"\n", true, 0),
    ] {
        let root = a_project_whose_policy_is(text);
        let said = bypass_refusal_in(root.path(), Some(chat), "yolo", &yolo, NamedBy::TheDispatch);
        assert_eq!(said.is_some(), refused, "{text}");
        let started = start_of_a_persona_chat_in(
            root.path(),
            Some(chat),
            the_asking_chats_own(),
            "devops",
            Some(Inherited {
                profile: "yolo",
                command: &yolo,
            }),
        );
        assert_eq!(started.is_err(), refused, "{text}");
        let logged = crate::enforcement::read_log(root.path(), chat);
        assert_eq!(logged.len(), lines * 2, "{text}");
        assert!(
            logged
                .iter()
                .all(|line| line.family == "mode" && line.rule == "dispatch.prompts-off")
        );
    }
    // A profile that asks is never refused, whatever the mode.
    let root = a_project_whose_policy_is("template = \"strict\"\n");
    assert_eq!(
        bypass_refusal_in(
            root.path(),
            None,
            "work",
            &words(&["claude"]),
            NamedBy::TheDispatch
        ),
        None
    );
}

#[test]
fn a_command_without_its_prompts_off_words_keeps_every_other_word() {
    // #7104: what a chat another chat asked for is started on, capped at auto.
    for (command, kept) in [
        (
            vec!["claude", "--dangerously-skip-permissions", "-v"],
            vec!["claude", "-v"],
        ),
        (
            vec!["claude", "--permission-mode", "bypassPermissions", "-v"],
            vec!["claude", "-v"],
        ),
        (vec!["codex", "-a", "never", "--yolo"], vec!["codex"]),
        (
            vec!["codex", "-capproval_policy=never", "exec"],
            vec!["codex", "exec"],
        ),
        (
            vec!["claude", "--permission-mode", "auto"],
            vec!["claude", "--permission-mode", "auto"],
        ),
    ] {
        let without = without_bypass(&words(&command));
        assert_eq!(without, words(&kept), "{command:?}");
        assert_eq!(bypass_in(&without), None, "{command:?}");
    }
}

#[test]
fn inline_settings_whose_default_mode_switches_the_prompts_off_lose_only_that_key() {
    // The re-review of #7104: Claude Code's `--settings` may carry its permission mode as
    // `permissions.defaultMode`; the cap at auto takes that key out and keeps the rest.
    let off = r#"{"permissions":{"defaultMode":"bypassPermissions","allow":["Read"]},"model":"x"}"#;
    let kept = r#"{"permissions":{"allow":["Read"]},"model":"x"}"#;
    for (command, want) in [
        (
            words(&["claude", "--settings", off, "-v"]),
            words(&["claude", "--settings", kept, "-v"]),
        ),
        (
            words(&["claude", &format!("--settings={off}")]),
            words(&["claude", &format!("--settings={kept}")]),
        ),
    ] {
        assert!(bypass_in(&command).is_some(), "{command:?}");
        let without = without_bypass(&command);
        assert_eq!(without, want, "{command:?}");
        assert_eq!(bypass_in(&without), None, "{command:?}");
    }
    // Settings that ask, and a settings file's path, are left as they are.
    for value in [
        r#"{"permissions":{"defaultMode":"auto"}}"#,
        "/etc/claude.json",
    ] {
        let command = words(&["claude", "--settings", value]);
        assert_eq!(bypass_in(&command), None, "{value}");
        assert_eq!(without_bypass(&command), command, "{value}");
    }
}

#[test]
fn inline_settings_purlis_cannot_read_are_refused_and_dropped_whole() {
    // #7155: JSON Claude Code accepts and `serde_json` rejects keeps no bypass unseen.
    for value in [
        "{\"permissions\":{\"defaultMode\":\"bypassPermissions\",}}",
        "{/* c */\"permissions\":{\"defaultMode\":\"bypassPermissions\"}}",
        "\u{feff}{\"permissions\":{\"defaultMode\":\"bypassPermissions\"}}",
        "{not json",
    ] {
        for command in [
            words(&["claude", "--settings", value, "-v"]),
            words(&["claude", &format!("--settings={value}"), "-v"]),
        ] {
            assert!(bypass_in(&command).is_some(), "{command:?}");
            let without = without_bypass(&command);
            assert_eq!(without, words(&["claude", "-v"]), "{command:?}");
            assert_eq!(bypass_in(&without), None, "{command:?}");
        }
    }
}

#[test]
fn inline_settings_are_read_through_whitespace_escaped_keys_and_repeated_flags() {
    let mode = "bypassPermissions";
    let spaced = format!(" \n {{\"permissions\" : {{ \"defaultMode\" : \"{mode}\" }} }}");
    // `defaultMode` spelt with an escape is the same key.
    let escaped = format!("{{\"permissions\":{{\"default\\u004dode\":\"{mode}\"}}}}");
    for value in [&spaced, &escaped] {
        let command = words(&["claude", "--settings", value]);
        assert!(bypass_in(&command).is_some(), "{value:?}");
        assert_eq!(bypass_in(&without_bypass(&command)), None, "{value:?}");
    }
    // The second of two `--settings` is the one that carries it, and the first is kept.
    let kept = r#"{"model":"x"}"#;
    let off = format!("{{\"permissions\":{{\"defaultMode\":\"{mode}\"}}}}");
    let command = words(&["claude", "--settings", kept, "--settings", &off]);
    assert!(bypass_in(&command).is_some());
    let without = without_bypass(&command);
    assert_eq!(bypass_in(&without), None);
    assert!(without.contains(&kept.to_owned()));
}

#[test]
fn inline_settings_that_allow_all_of_bash_or_silence_hooks_are_refused_and_lose_only_those_keys() {
    // #7165: no `defaultMode`, and still no prompts for Bash, or no guards.
    for (value, kept, shown) in [
        (
            r#"{"permissions":{"allow":["Bash","Read"]},"model":"x"}"#,
            r#"{"permissions":{"allow":["Read"]},"model":"x"}"#,
            "permissions.allow of Bash",
        ),
        (
            r#"{"permissions":{"allow":["Bash(*)","Bash(:*)","Bash( ** )"]}}"#,
            r#"{"permissions":{"allow":[]}}"#,
            "permissions.allow of Bash",
        ),
        (
            r#"{"hooks":{"PreToolUse":[]},"model":"x"}"#,
            r#"{"model":"x"}"#,
            "hooks",
        ),
        (
            r#"{"disableAllHooks":true,"model":"x"}"#,
            r#"{"model":"x"}"#,
            "disableAllHooks",
        ),
        (r#"{"disableAllHooks":"yes"}"#, "{}", "disableAllHooks"),
    ] {
        for (command, want) in [
            (
                words(&["claude", "--settings", value, "-v"]),
                words(&["claude", "--settings", kept, "-v"]),
            ),
            (
                words(&["claude", &format!("--settings={value}")]),
                words(&["claude", &format!("--settings={kept}")]),
            ),
        ] {
            let said = bypass_in(&command).unwrap_or_default();
            assert!(said.contains(shown), "{value}: {said}");
            let without = without_bypass(&command);
            assert_eq!(without, want, "{value}");
            assert_eq!(bypass_in(&without), None, "{value}");
        }
    }
    // A narrow allow of Bash asks for everything else, so it stays.
    let narrow = words(&[
        "claude",
        "--settings",
        r#"{"permissions":{"allow":["Bash(git status)","Bash(npm run *)"]}}"#,
    ]);
    assert_eq!(bypass_in(&narrow), None);
    assert_eq!(without_bypass(&narrow), narrow);
}

#[test]
fn an_unreadable_inline_settings_value_does_not_say_it_switches_the_prompts_off() {
    // #7165, item 6: it is refused because nothing can be ruled out, and says so.
    let command = words(&["claude", "--settings", "{not json"]);
    let said = bypass_refusal("work", &command, NamedBy::TheDispatch).expect("refused");
    assert!(said.contains("cannot tell whether"), "{said}");
    assert!(!said.contains("with the permission prompts off"), "{said}");
}

#[test]
fn the_settings_purlis_armed_keep_their_hooks_through_without_bypass() {
    // The round-2 blocker of #7168: purlis's own `--settings` always carry `hooks`, and the line
    // a chat starts on holds them beside the profile's words.
    let own = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/bin/purlis hook bash-guard","onFailure":"block"}]}]},"env":{"X":"1"}}"#;
    let armed = words(&["--plugin-dir", "/p", "--settings", own]);
    let mut line = words(&["claude", "--model", "x"]);
    line.extend(armed.clone());
    assert_eq!(bypass_found_beside(&line, &armed), None);
    assert_eq!(without_bypass_beside(&line, &armed), line);
    // Attached, the same.
    let attached = words(&[&format!("--settings={own}")]);
    assert_eq!(
        bypass_found_beside(&attached, &[]).map(|f| f.what),
        Some(What::GuardsOff)
    );
    // Read as a profile's, the same words are flagged and lose their hooks: only the exact
    // words purlis armed are exempt, and a copy that differs in a byte is not.
    assert!(bypass_in(&line).is_some());
    assert!(
        !without_bypass(&line)
            .iter()
            .any(|w| w.contains("bash-guard"))
    );
    let spoof = words(&["--settings", &format!("{own} ")]);
    assert!(bypass_found_beside(&spoof, &armed).is_some());
    // And a profile's own hooks beside purlis's are still taken out.
    let mut both = armed.clone();
    both.extend(words(&["--settings", r#"{"hooks":{"PreToolUse":[]}}"#]));
    let without = without_bypass_beside(&both, &armed);
    assert!(without.contains(&own.to_owned()));
    assert!(without.contains(&"{}".to_owned()));
}

#[test]
fn a_profile_that_only_adds_a_hook_is_refused_in_words_that_are_true_of_it() {
    // Round 2, item 6: the refusal said the prompts were off, and clipped "cannot tell".
    let hook = words(&["claude", "--settings", r#"{"hooks":{"PreToolUse":[]}}"#]);
    let long = words(&[
        "claude",
        "--settings",
        r#"{"hooks":{},"disableAllHooks":true,"allowManagedHooksOnly":true}"#,
    ]);
    let unreadable = words(&["claude", "--settings", "{not json"]);
    for (command, must) in [
        (&hook, "purlis's guards off (--settings (its hooks))"),
        (&long, "purlis's guards off (--settings (its hooks"),
        (
            &unreadable,
            "cannot tell whether they switch the permission prompts or its guards off (--settings (unreadable))",
        ),
    ] {
        for by in [
            NamedBy::TheAskingChat,
            NamedBy::TheDispatch,
            NamedBy::ThePersona("p"),
        ] {
            let said = bypass_refusal("work", command, by).expect("refused");
            assert!(said.contains(must), "{said}");
        }
    }
    let said = bypass_refusal("work", &hook, NamedBy::TheDispatch).expect("refused");
    assert!(!said.contains("with the permission prompts off"), "{said}");
    // A flag that does switch them off reads as it always did.
    let yolo = words(&["claude", "--dangerously-skip-permissions"]);
    let said = bypass_refusal("work", &yolo, NamedBy::TheDispatch).expect("refused");
    assert!(
        said.contains("with the permission prompts off (--dangerously-skip-permissions)"),
        "{said}"
    );
}

#[test]
fn inline_settings_that_move_a_guard_or_the_sandbox_gate_lose_only_those_keys() {
    // Round 2, item 3.
    for (value, kept, shown) in [
        (
            r#"{"allowManagedHooksOnly":true,"model":"x"}"#,
            r#"{"model":"x"}"#,
            "allowManagedHooksOnly",
        ),
        // Round 3: the key is written `false`, never removed (removing it restores Claude Code's
        // default, which is on), and an enabled sandbox with no key is the same finding.
        (
            r#"{"sandbox":{"autoAllowBashIfSandboxed":true,"enabled":true}}"#,
            r#"{"sandbox":{"autoAllowBashIfSandboxed":false,"enabled":true}}"#,
            "sandbox.autoAllowBashIfSandboxed",
        ),
        (
            r#"{"sandbox":{"enabled":true}}"#,
            r#"{"sandbox":{"enabled":true,"autoAllowBashIfSandboxed":false}}"#,
            "sandbox.autoAllowBashIfSandboxed",
        ),
        (
            r#"{"sandbox":{"autoAllowBashIfSandboxed":"yes"}}"#,
            r#"{"sandbox":{"autoAllowBashIfSandboxed":false}}"#,
            "sandbox.autoAllowBashIfSandboxed",
        ),
        (
            r#"{"env":{"HOME":"/x","KEEP":"1"}}"#,
            r#"{"env":{"KEEP":"1"}}"#,
            "env that moves its guards",
        ),
        (
            r#"{"env":{"claude_config_dir":"/x","PURLIS_SOCK":"/y","CHARTER_APP":"1"}}"#,
            r#"{"env":{}}"#,
            "env that moves its guards",
        ),
    ] {
        let command = words(&["claude", "--settings", value]);
        let said = bypass_in(&command).unwrap_or_default();
        assert!(said.contains(shown), "{value}: {said}");
        assert_eq!(
            without_bypass(&command),
            words(&["claude", "--settings", kept]),
            "{value}"
        );
        assert_eq!(bypass_in(&without_bypass(&command)), None, "{value}");
    }
    // An ordinary env and a sandbox that does not auto-allow stay.
    for value in [
        r#"{"env":{"FOO":"1"}}"#,
        r#"{"sandbox":{"autoAllowBashIfSandboxed":false}}"#,
        r#"{"sandbox":{"enabled":true,"autoAllowBashIfSandboxed":false}}"#,
        // A sandbox this does not enable is not read as one that allows.
        r#"{"sandbox":{"enabled":false}}"#,
        r#"{"sandbox":{}}"#,
    ] {
        let command = words(&["claude", "--settings", value]);
        assert_eq!(bypass_in(&command), None, "{value}");
        assert_eq!(without_bypass(&command), command, "{value}");
    }
    // `--safe-mode` is a flag of its own, and goes alone.
    let command = words(&["claude", "--safe-mode", "-v"]);
    assert_eq!(bypass_in(&command).as_deref(), Some("--safe-mode"));
    // It narrows hooks, so it is a guard, not a prompt.
    assert_eq!(
        bypass_found(&command).map(|found| found.what),
        Some(What::GuardsOff)
    );
    assert_eq!(without_bypass(&command), words(&["claude", "-v"]));
}

#[test]
fn a_broad_bash_allow_is_read_without_regard_to_case_or_whitespace() {
    for rule in [
        "bash",
        "BASH",
        " Bash ",
        "Bash (*)",
        "bash(*)",
        "Bash(\t*\t)",
        "Bash( : * )",
        "Bash(**)",
        "Bash(\u{a0}*)",
    ] {
        let value = format!(
            r#"{{"permissions":{{"allow":[{}]}}}}"#,
            serde_json::json!(rule)
        );
        let command = words(&["claude", "--settings", &value]);
        assert!(bypass_in(&command).is_some(), "{rule:?}");
        assert_eq!(bypass_in(&without_bypass(&command)), None, "{rule:?}");
    }
    for rule in ["Bash(git status)", "Bash(npm run *)", "Bashful", "Read(*)"] {
        let value = format!(
            r#"{{"permissions":{{"allow":[{}]}}}}"#,
            serde_json::json!(rule)
        );
        assert_eq!(
            bypass_in(&words(&["claude", "--settings", &value])),
            None,
            "{rule:?}"
        );
    }
}
