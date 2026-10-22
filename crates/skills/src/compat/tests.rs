// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The compatibility badge and the `SKILL.md` purlis hands over, on harnesses described the way
//! each adapter describes its own (SK-8, #2771).

use super::*;

/// A harness that reads every key a skill may set, and manual-only itself.
const READS_ALL: Honours = Honours {
    reads: &[
        Field::WhenToUse,
        Field::ManualOnly,
        Field::UserInvocable,
        Field::AllowedTools,
        Field::DisallowedTools,
        Field::Model,
        Field::Effort,
        Field::Context,
        Field::Agent,
        Field::Hooks,
        Field::Paths,
        Field::Shell,
        Field::Background,
    ],
    manual: ManualOnly::Read,
};

/// A harness purlis lists the skills to.
const BRIEFED: Honours = Honours {
    reads: &[],
    manual: ManualOnly::Listing,
};

/// A harness that lists only a skill with a description.
const BY_DESCRIPTION: Honours = Honours {
    reads: &[],
    manual: ManualOnly::WithoutDescription,
};

const NONE: Honours = Honours {
    reads: &[],
    manual: ManualOnly::Not,
};

fn skill(more: &str) -> String {
    format!("---\nname: deploy\ndescription: Deploys the app.\n{more}---\n\nSteps.\n")
}

#[test]
fn a_skill_that_sets_only_its_name_and_description_fits_every_harness() {
    for honours in [READS_ALL, BRIEFED, BY_DESCRIPTION, NONE] {
        let badge = compat(&skill(""), true, &honours);
        assert_eq!(badge.fit, Fit::Full, "{honours:?}");
        assert_eq!(badge.lines(), ["Everything it asks for applies."]);
    }
}

#[test]
fn what_a_harness_does_not_read_is_on_its_badge_and_a_narrowing_says_it_is_not_applied() {
    let text = skill("model: opus\neffort: high\ndisallowed-tools: Write, Edit\ncontext: fork\n");
    assert_eq!(compat(&text, true, &READS_ALL).fit, Fit::Full);
    let badge = compat(&text, true, &BRIEFED);
    assert_eq!(badge.fit, Fit::Partial);
    assert_eq!(
        badge.ignored,
        [
            Field::Model,
            Field::Effort,
            Field::DisallowedTools,
            Field::Context
        ]
    );
    assert_eq!(
        badge.lines(),
        [
            "Narrowing not applied: the tools it takes away and running in a fork of the chat.",
            "Ignored: the model it asks for and the effort it asks for.",
        ]
    );
}

#[test]
fn the_arguments_apply_everywhere_as_use_skill_asks_for_them() {
    let text = skill("arguments: [pr, focus]\nargument-hint: <pr>\n");
    for honours in [READS_ALL, NONE] {
        assert_eq!(compat(&text, true, &honours).fit, Fit::Full);
    }
}

#[test]
fn a_skill_purlis_hands_over_is_never_handed_what_widens_the_chat_and_its_badge_says_so() {
    let text = skill(
        "allowed-tools: Bash(rm:*)\nhooks:\n  PreToolUse:\n    - matcher: Bash\n      hooks:\n        - type: command\n          command: ./x.sh\ndisallowed-tools: Write\n",
    );
    for honours in [READS_ALL, BRIEFED, BY_DESCRIPTION] {
        let badge = compat(&text, true, &honours);
        assert_eq!(badge.dropped, [Field::AllowedTools, Field::Hooks]);
        assert_eq!(badge.fit, Fit::Partial);
        assert!(badge.lines()[0].starts_with(
            "Left out, because a skill may only narrow a chat: the tools it pre-approves and \
             the commands it adds"
        ));
        let handed = handed_text(&text, &honours).expect("rewritten");
        assert_eq!(
            handed,
            skill("disallowed-tools: Write\n"),
            "the narrowing passes through"
        );
    }
}

#[test]
fn a_harness_or_repo_skill_that_widens_the_chat_is_marked_where_its_harness_applies_it() {
    let text = skill("allowed-tools: Bash\n");
    let badge = compat(&text, false, &READS_ALL);
    assert_eq!(badge.fit, Fit::Widens);
    assert_eq!(badge.widens, [Field::AllowedTools]);
    assert!(badge.lines()[0].starts_with("Widens the chat: the tools it pre-approves apply"));
    let elsewhere = compat(&text, false, &NONE);
    assert_eq!(elsewhere.fit, Fit::Partial);
    assert_eq!(elsewhere.ignored, [Field::AllowedTools]);
}

#[test]
fn manual_only_is_read_translated_or_said_to_be_ignored() {
    let text = skill("disable-model-invocation: true\n");
    assert_eq!(compat(&text, true, &READS_ALL).fit, Fit::Full);
    for honours in [BRIEFED, BY_DESCRIPTION] {
        let badge = compat(&text, true, &honours);
        assert_eq!(badge.fit, Fit::Full);
        assert_eq!(badge.translated, [Field::ManualOnly]);
        assert_eq!(
            badge.lines(),
            ["Carried over by purlis's equivalent: running only when asked for."]
        );
    }
    assert_eq!(compat(&text, true, &NONE).ignored, [Field::ManualOnly]);
    // purlis translates only what it hands over.
    assert_eq!(compat(&text, false, &BRIEFED).ignored, [Field::ManualOnly]);
    // And `false` asks for nothing.
    let off = skill("disable-model-invocation: false\n");
    assert_eq!(compat(&off, true, &NONE).fit, Fit::Full);
}

#[test]
fn a_manual_only_skill_is_handed_without_its_description_only_where_that_keeps_it_out() {
    let text = skill("disable-model-invocation: yes\n");
    assert_eq!(
        handed_text(&text, &BY_DESCRIPTION).as_deref(),
        Some("---\nname: deploy\ndisable-model-invocation: yes\n---\n\nSteps.\n")
    );
    for honours in [READS_ALL, BRIEFED, NONE] {
        assert_eq!(handed_text(&text, &honours), None, "{honours:?}");
    }
    assert_eq!(handed_text(&skill(""), &BY_DESCRIPTION), None);
}

#[test]
fn a_key_nothing_reads_is_said_and_a_readers_key_is_not() {
    let text = skill("license: MIT\nmetadata:\n  team: ops\nowner: ops\n");
    let badge = compat(&text, true, &READS_ALL);
    assert_eq!(badge.unknown, ["owner"]);
    assert_eq!(badge.fit, Fit::Partial);
    assert_eq!(badge.lines(), ["Read by nothing: owner."]);
}

#[test]
fn manual_only_is_kept_out_wherever_a_harness_has_a_way() {
    assert!(manual_kept_out(&READS_ALL));
    assert!(manual_kept_out(&BRIEFED));
    assert!(manual_kept_out(&BY_DESCRIPTION));
    assert!(!manual_kept_out(&NONE));
}
