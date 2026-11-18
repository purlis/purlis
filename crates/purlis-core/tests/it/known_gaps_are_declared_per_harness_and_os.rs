// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Known gaps, declared per harness and OS** (M75 slice 37, #7046; spec §6, §14.8; Q319 C1).
//!
//! A rule in force that a harness cannot enforce is a known gap: declared, never refused, unless
//! the project writes the refusal as a forbid on `start`. These tests judge the capability table
//! against the schema, the entries against #6972 §4, and a start's refusal against a real plane
//! made with git in a temporary folder.

use std::collections::BTreeSet;
use std::path::Path;

use purlis_core::enforcement::{Family, Mode, Resolved};
use purlis_core::harness::Harness;
use purlis_core::knowngaps::{self, Cover, Declared, Facts, GAPS, Judged};
use purlis_core::policy::levels::Chat;
use purlis_core::sandbox::Os;

const SYSTEMS: [Os; 3] = [Os::MacOs, Os::Linux, Os::Windows];

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(["-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane whose root commits `policy.cedar` with `rules`, or none.
fn plane(rules: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("README.md"), "a plane\n").expect("a file");
    if let Some(rules) = rules {
        std::fs::write(
            dir.path().join("policy.cedar"),
            format!("// purlis-policy 1\n{rules}"),
        )
        .expect("a file");
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn facts(harness: Harness, os: Os) -> Facts {
    Facts {
        harness,
        os,
        sandboxed: true,
        claude_code_older: false,
        plane_root_rename_held: false,
    }
}

fn judged(root: &Path) -> Judged<'_> {
    Judged {
        root,
        profile: "claude",
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "floor",
    }
}

/// `floor` with `family` at `mode`, every other family enforced.
fn floor_with(family: Family, mode: Mode) -> Resolved {
    let text = Family::ALL
        .into_iter()
        .map(|each| {
            let mode = if each == family { mode } else { Mode::Enforce };
            format!("{}={}", each.key(), mode.word())
        })
        .collect::<Vec<_>>()
        .join(",");
    let parsed = Resolved::parse(&text);
    assert_eq!(parsed.mode(family), mode, "the floor spells {text}");
    parsed
}

const REFUSE_ON_A_SHAPE_GAP: &str = r#"
@id("trust-no-start-on-a-shape-gap") @level("project")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("guards.shape") };
"#;

#[test]
fn a_start_whose_policy_needs_a_hook_the_harness_lacks_declares_the_gap_and_starts() {
    // Codex has no fail-closed point for the shape of a call but its hook, and the shipped
    // default enforces it.
    let plane = plane(None);
    let floor = Resolved::all(Mode::Enforce);
    let facts = facts(Harness::Codex, Os::Linux);
    let declared = Declared::at(&facts, &floor);
    assert!(declared.ids().contains(&knowngaps::HOOK_FAILS_OPEN));
    assert!(declared.context_gaps().contains(&"guards.shape".to_owned()));
    // The shipped default has no rule on `start`: the chat starts.
    assert_eq!(
        knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor),
        None,
        "no known gap refuses a start under the shipped default"
    );
}

#[test]
fn a_project_rule_on_start_with_a_gap_in_an_enforced_family_refuses_naming_the_rule() {
    let plane = plane(Some(REFUSE_ON_A_SHAPE_GAP));
    let floor = Resolved::all(Mode::Enforce);
    let facts = facts(Harness::Codex, Os::Linux);
    let declared = Declared::at(&facts, &floor);
    let refused = knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor)
        .expect("the project's rule refuses a start with a gap in a family it enforces");
    assert!(
        refused.contains("trust-no-start-on-a-shape-gap"),
        "names the rule: {refused}"
    );
    assert!(
        refused.contains(knowngaps::HOOK_FAILS_OPEN),
        "names the gap: {refused}"
    );
    assert!(refused.starts_with("Nothing was started."), "{refused}");
}

#[test]
fn a_gap_in_a_family_set_to_log_or_off_is_not_in_context_gaps() {
    let facts = facts(Harness::Codex, Os::Linux);
    let enforced = Declared::at(&facts, &Resolved::all(Mode::Enforce));
    assert!(enforced.context_gaps().contains(&"guards.shape".to_owned()));

    for mode in [Mode::Log, Mode::Off] {
        let declared = Declared::at(&facts, &floor_with(Family::GuardsShape, mode));
        assert!(
            !declared.context_gaps().contains(&"guards.shape".to_owned()),
            "a family at {mode:?} is not enforced, so it is not in context.gaps"
        );
    }

    // And a project rule that refuses on it refuses no start.
    let plane = plane(Some(REFUSE_ON_A_SHAPE_GAP));
    for mode in [Mode::Log, Mode::Off] {
        let floor = floor_with(Family::GuardsShape, mode);
        let declared = Declared::at(&facts, &floor);
        assert_eq!(
            knowngaps::refusal(&judged(plane.path()), &facts, &declared, &floor),
            None
        );
    }
}

#[test]
fn an_off_rule_needs_no_enforcement_and_declares_no_gap() {
    let facts = facts(Harness::Codex, Os::Linux);
    let off = Declared::at(&facts, &floor_with(Family::Asks, Mode::Off));
    assert!(
        off.entries()
            .iter()
            .all(|entry| !entry.in_force.contains(&Family::Asks)),
        "no declared gap names the family that is off"
    );
    // The Codex gaps that affect only `asks` are gone, and the others stay.
    for id in ["codex-no-command-ask", "codex-sandboxed-asks-rejected"] {
        assert!(!off.ids().contains(&id), "{id} needs enforcement of `asks`");
    }
    let on = Declared::at(&facts, &Resolved::all(Mode::Enforce));
    for id in ["codex-no-command-ask", "codex-sandboxed-asks-rejected"] {
        assert!(on.ids().contains(&id), "{id} is declared where `asks` is on");
    }
    // A `log` family still declares: it is in force, only not enforced.
    let logging = Declared::at(&facts, &floor_with(Family::Asks, Mode::Log));
    assert!(logging.ids().contains(&"codex-no-command-ask"));
}

#[test]
fn every_gap_of_the_harness_research_has_an_entry() {
    let numbered: Vec<u8> = GAPS.iter().filter_map(|gap| gap.research).collect();
    assert_eq!(
        numbered,
        (1..=24).collect::<Vec<u8>>(),
        "#6972 §4's 24 gaps, each once, in its order"
    );
    let ids: BTreeSet<&str> = GAPS.iter().map(|gap| gap.id).collect();
    assert_eq!(ids.len(), GAPS.len(), "each gap has an id of its own");
    for gap in GAPS {
        assert!(!gap.said.is_empty() && !gap.instead.is_empty() && !gap.from.is_empty());
        assert!(
            gap.id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "{} is kebab-case",
            gap.id
        );
    }
}

#[test]
fn every_action_has_a_fail_closed_point_or_a_gap() {
    let actions = purlis_core::policy::actions();
    assert_eq!(actions.len(), 51, "the schema's actions: {actions:?}");
    for (action, family) in actions {
        let family = Family::of_key(family).expect("a family of the schema");
        for harness in Harness::ALL {
            for os in SYSTEMS {
                let cover = knowngaps::cover(action, harness, os).unwrap_or_else(|| {
                    panic!("`{action}` has no row in the capability table ({harness:?}, {os:?})")
                });
                if let Cover::Gap(id) = cover {
                    let gap = GAPS
                        .iter()
                        .find(|gap| gap.id == id)
                        .unwrap_or_else(|| panic!("`{action}`'s gap `{id}` has no entry"));
                    assert!(
                        gap.families.contains(&family),
                        "`{id}` does not say it affects `{}`, which holds `{action}`",
                        family.key()
                    );
                    assert!(
                        gap.may_hold_for(harness, os),
                        "`{id}` stands for `{action}` on {harness:?} on {os:?} but does not hold there"
                    );
                }
            }
        }
    }
    assert_eq!(knowngaps::cover("no-such-action", Harness::Codex, Os::Linux), None);
}

#[test]
fn claude_code_reads_are_a_declared_gap() {
    let floor = Resolved::all(Mode::Enforce);
    for os in [Os::MacOs, Os::Linux] {
        let declared = Declared::at(&facts(Harness::ClaudeCode, os), &floor);
        assert!(declared.ids().contains(&"claude-code-reads"));
        assert!(declared.context_gaps().contains(&"sandbox".to_owned()));
    }
    let codex = Declared::at(&facts(Harness::Codex, Os::MacOs), &floor);
    assert!(!codex.ids().contains(&"claude-code-reads"));
}

#[test]
fn an_unheld_plane_root_rename_is_a_declared_gap() {
    let floor = Resolved::all(Mode::Enforce);
    let mut unheld = facts(Harness::Codex, Os::Linux);
    assert!(Declared::at(&unheld, &floor).ids().contains(&"plane-root-rename"));
    unheld.plane_root_rename_held = true;
    assert!(!Declared::at(&unheld, &floor).ids().contains(&"plane-root-rename"));
}

#[test]
fn on_windows_an_unset_context_machine_is_a_declared_gap_and_a_rule_reading_it_denies() {
    let floor = Resolved::all(Mode::Enforce);
    let on_windows = Declared::at(&facts(Harness::ClaudeCode, Os::Windows), &floor);
    let gap = on_windows
        .entries()
        .iter()
        .find(|entry| entry.gap.id == "machine-unset-on-windows")
        .expect("declared on Windows");
    assert!(gap.gap.families.is_empty(), "no family's mode switches it off");
    for os in [Os::MacOs, Os::Linux] {
        let there = Declared::at(&facts(Harness::ClaudeCode, os), &floor);
        assert!(!there.ids().contains(&"machine-unset-on-windows"));
    }

    // A rule that reads `context.machine` (guarded, as the schema's optional attribute needs),
    // with the device id unset as it is on Windows, denies: what it expects is never there.
    let plane = plane(Some(
        r#"
@id("processes-this-machine-only") @level("project")
forbid (principal, action == Action::"exec", resource)
  unless { context has machine && context.machine == "01JABCDEFGHJKMNPQRSTVWXYZ0" };
"#,
    ));
    let asker = purlis_core::policy::decide::Asker {
        chat: 1,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
    };
    let decision = purlis_core::policy::decide::in_plane(
        plane.path(),
        &asker,
        "exec",
        ("Command", "ls"),
    );
    assert_eq!(
        decision.outcome,
        purlis_core::policy::decide::Outcome::Deny,
        "a rule that reads an unset machine denies: {decision:?}"
    );
    assert!(decision.ids().contains(&"processes-this-machine-only"), "{decision:?}");
}

#[test]
fn a_declared_gap_shows_in_settings_doctor_and_the_pane_head() {
    // Doctor declares each entry that holds for a harness on this system. Settings and the pane
    // head read the same list through the app's commands; their stories and tests are the
    // window's (`KnownGaps.test.tsx`).
    let rows = purlis_core::doctor::known_gaps_rows(&Resolved::all(Mode::Enforce), Os::Linux);
    assert_eq!(rows.len(), Harness::ALL.len(), "a row for each harness");
    for harness in Harness::ALL {
        let row = rows
            .iter()
            .find(|row| row.name == format!("known gaps: {}", harness.name()))
            .unwrap_or_else(|| panic!("doctor has no row for {harness:?}"));
        for gap in GAPS.iter().filter(|gap| gap.may_hold_for(harness, Os::Linux)) {
            assert!(
                row.detail.contains(gap.id) && row.detail.contains(gap.said),
                "doctor does not declare `{}` for {harness:?}: {row:?}",
                gap.id
            );
        }
        assert_ne!(
            row.status,
            purlis_core::doctor::Status::Fail,
            "a known gap never fails a start or a doctor run"
        );
    }
    // A gap of a family that is off is not declared.
    let off = purlis_core::doctor::known_gaps_rows(&floor_with(Family::Asks, Mode::Off), Os::Linux);
    let codex = off
        .iter()
        .find(|row| row.name == "known gaps: codex")
        .expect("a row for codex");
    assert!(!codex.detail.contains("codex-no-command-ask"), "{codex:?}");
}

