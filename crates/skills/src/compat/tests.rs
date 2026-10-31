// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The compatibility badge and the `SKILL.md` purlis hands over, on harnesses described the way
//! each adapter describes its own (SK-8, #2771).

use super::*;

const HANDED: Origin = Origin::Project;
const OUTSIDE: Origin = Origin::Repo;

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
    agents: &["general-purpose"],
};

/// A harness purlis lists the skills to.
const BRIEFED: Honours = Honours {
    reads: &[],
    manual: ManualOnly::Listing,
    agents: &["general-purpose"],
};

/// A harness that lists only a skill with a description.
const BY_DESCRIPTION: Honours = Honours {
    reads: &[],
    manual: ManualOnly::WithoutDescription,
    agents: &["general-purpose"],
};

fn skill(more: &str) -> String {
    format!("---\nname: deploy\ndescription: Deploys the app.\n{more}---\n\nSteps.\n")
}

#[test]
fn a_skill_that_sets_only_its_name_and_description_fits_every_harness() {
    for honours in [READS_ALL, BRIEFED, BY_DESCRIPTION] {
        let badge = compat(&skill(""), HANDED, &honours);
        assert_eq!(badge.fit, Fit::Full, "{honours:?}");
        assert_eq!(badge.lines(), ["Everything it asks for applies."]);
    }
}

#[test]
fn what_a_harness_does_not_read_is_on_its_badge_and_a_narrowing_says_it_is_not_applied() {
    let text = skill("model: opus\neffort: high\ndisallowed-tools: Write, Edit\ncontext: fork\n");
    assert_eq!(compat(&text, HANDED, &READS_ALL).fit, Fit::Full);
    let badge = compat(&text, HANDED, &BRIEFED);
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
    for honours in [READS_ALL, BRIEFED] {
        assert_eq!(compat(&text, HANDED, &honours).fit, Fit::Full);
    }
}

#[test]
fn a_skill_purlis_hands_over_is_never_handed_what_widens_the_chat_and_its_badge_says_so() {
    let text = skill(
        "allowed-tools: Bash(rm:*)\nhooks:\n  PreToolUse:\n    - matcher: Bash\n      hooks:\n        - type: command\n          command: ./x.sh\ndisallowed-tools: Write\n",
    );
    for honours in [READS_ALL, BRIEFED, BY_DESCRIPTION] {
        let badge = compat(&text, HANDED, &honours);
        assert_eq!(badge.dropped, [Field::AllowedTools, Field::Hooks]);
        assert_eq!(badge.fit, Fit::Partial);
        assert!(badge.lines()[0].starts_with(
            "Left out, because a skill may only narrow a chat: the tools it pre-approves and \
             the commands it adds"
        ));
        let handed = handed_text(&text, &honours).expect("handed");
        assert_eq!(
            handed,
            "---\nname: deploy\ndescription: Deploys the app.\ndisallowed-tools: Write\n---\n\nSteps.\n",
            "the narrowing passes through"
        );
    }
}

#[test]
fn a_harness_or_repo_skill_that_widens_the_chat_is_marked_where_its_harness_applies_it() {
    let text = skill("allowed-tools: Bash\n");
    let badge = compat(&text, OUTSIDE, &READS_ALL);
    assert_eq!(badge.fit, Fit::Widens);
    assert_eq!(badge.widens, [Field::AllowedTools]);
    assert!(badge.lines()[0].starts_with("Widens the chat: the tools it pre-approves apply"));
    let elsewhere = compat(&text, OUTSIDE, &BRIEFED);
    assert_eq!(elsewhere.fit, Fit::Partial);
    assert_eq!(elsewhere.ignored, [Field::AllowedTools]);
}

#[test]
fn manual_only_is_read_translated_or_said_to_be_ignored() {
    let text = skill("disable-model-invocation: true\n");
    let read = compat(&text, HANDED, &READS_ALL);
    assert_eq!(read.fit, Fit::Full);
    assert_eq!(
        read.lines(),
        [
            "Everything it asks for applies.",
            "Runs only when asked for by name."
        ],
        "a harness that reads it itself says so"
    );
    for honours in [BRIEFED, BY_DESCRIPTION] {
        let badge = compat(&text, HANDED, &honours);
        assert_eq!(badge.fit, Fit::Full);
        assert_eq!(badge.translated, [Field::ManualOnly]);
        assert_eq!(
            badge.lines(),
            ["Carried over by purlis's equivalent: running only when asked for."]
        );
    }
    // purlis translates only what it hands over.
    assert_eq!(
        compat(&text, OUTSIDE, &BRIEFED).ignored,
        [Field::ManualOnly]
    );
    // And `false` asks for nothing.
    let off = skill("disable-model-invocation: false\n");
    assert_eq!(compat(&off, HANDED, &BRIEFED).fit, Fit::Full);
}

#[test]
fn a_manual_only_skill_is_handed_without_its_description_only_where_that_keeps_it_out() {
    let text = skill("disable-model-invocation: yes\n");
    assert_eq!(
        handed_text(&text, &BY_DESCRIPTION).as_deref(),
        Ok("---\nname: deploy\ndisable-model-invocation: true\n---\n\nSteps.\n")
    );
    for honours in [READS_ALL, BRIEFED] {
        assert!(
            handed_text(&text, &honours)
                .unwrap()
                .contains("description: Deploys the app.\n"),
            "{honours:?}"
        );
    }
}

#[test]
fn a_key_nothing_reads_is_said_and_a_readers_key_is_not() {
    let text = skill("license: MIT\nmetadata:\n  team: ops\nowner: ops\n");
    let badge = compat(&text, HANDED, &READS_ALL);
    assert_eq!(badge.unknown, 1);
    assert_eq!(badge.fit, Fit::Partial);
    assert_eq!(badge.lines(), ["Some of what it sets is read by nothing."]);
}

#[test]
fn a_frontmatter_purlis_cannot_read_is_refused_when_handed_and_may_widen_when_not() {
    let text = "---\nname: deploy\nallowedTools: Bash\n---\n";
    let handed = compat(text, HANDED, &READS_ALL);
    assert_eq!(handed.fit, Fit::Refused);
    assert!(
        handed.lines()[0].starts_with("purlis hands it to no chat: "),
        "{:?}",
        handed.lines()
    );
    let outside = compat(text, OUTSIDE, &READS_ALL);
    assert_eq!(outside.fit, Fit::Widens);
    assert!(outside.lines()[0].starts_with("It may widen the chat"));
}

#[test]
fn an_agent_that_is_not_the_harnesss_own_is_left_out_and_the_badge_says_so() {
    let text = skill("context: fork\nagent: repo-reviewer\n");
    let badge = compat(&text, HANDED, &READS_ALL);
    assert_eq!(badge.dropped, [Field::Agent]);
    assert_eq!(
        badge.lines(),
        ["Left out, because a skill may only narrow a chat: what its fork runs as."]
    );
    assert!(!handed_text(&text, &READS_ALL).unwrap().contains("agent:"));
    let own = skill("context: fork\nagent: general-purpose\n");
    assert_eq!(compat(&own, HANDED, &READS_ALL).fit, Fit::Full);
}

#[test]
fn a_skill_its_harness_keeps_manual_only_by_a_file_beside_it_says_so_on_its_badge() {
    // SK-39: Codex's own skills, read where they lie, are kept manual-only by a file of Codex's
    // beside the `SKILL.md`. The frontmatter key it does not read is then no longer ignored.
    let plain = compat(&skill(""), OUTSIDE, &BRIEFED).kept_manual();
    assert_eq!(plain.fit, Fit::Full);
    assert_eq!(
        plain.lines(),
        [
            "Everything it asks for applies.",
            "Runs only when asked for by name."
        ]
    );
    let keyed = compat(
        &skill("disable-model-invocation: true\n"),
        OUTSIDE,
        &BRIEFED,
    );
    assert_eq!(keyed.fit, Fit::Partial);
    let keyed = keyed.kept_manual();
    assert_eq!(keyed.fit, Fit::Full);
    assert!(keyed.ignored.is_empty(), "{keyed:?}");
    // What else it asks for is still said.
    let model = compat(&skill("model: opus\n"), OUTSIDE, &BRIEFED).kept_manual();
    assert_eq!(model.fit, Fit::Partial);
    assert_eq!(
        model.lines(),
        [
            "Runs only when asked for by name.",
            "Ignored: the model it asks for."
        ]
    );
    // One purlis cannot read stays refused.
    let refused = compat("no frontmatter", OUTSIDE, &BRIEFED);
    assert_eq!(refused.clone().kept_manual(), refused);
}

#[test]
fn a_harness_with_no_prompt_hook_says_no_router_points_a_chat_there() {
    // SK-15c (#3231): a router that reads prompts points a prompt at skills the chat's list
    // leaves out through the harness's prompt hook; where it has none, the badge says so.
    let badge = compat(&skill(""), HANDED, &READS_ALL);
    assert!(
        !badge
            .lines()
            .iter()
            .any(|line| line.contains("prompt hook")),
        "{:?}",
        badge.lines()
    );
    let without = badge.without_a_pointer();
    let lines = without.lines();
    assert!(
        lines.iter().any(|line| line
            == "No router points a prompt at it here: this program has no prompt hook, so the chat \
                finds it in its list, the index skill or with skill_find."),
        "{lines:?}"
    );
    assert_eq!(without.fit, Fit::Full, "how well it fits is not changed");
}
