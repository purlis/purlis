// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one decision function** (M75 slice 04, #7012; spec §6 and §13.1).
//!
//! `decide` judges a request against the policy as every level wrote it, committed at a real
//! plane's `HEAD`, and says the outcome, the rules that decided it (with their level and owner),
//! the family, the mode and the outcome after it. The probe policies of spec §14.6 that are this
//! slice's are here: ask beats allow, deny beats ask, a narrower permit never punches through an
//! ask, an ask with no permit beneath it is a deny naming `policy-everything-else (off)`, a forbid
//! inside a permit, and an evaluation error that denies.

use std::path::Path;

use purlis_core::enforcement::Mode;
use purlis_core::policy::decide::{self, Decision, Outcome, Request};
use purlis_core::policy::levels::{self, Chat, Loaded, Sources};
use purlis_core::policy::{Question, World};

const WORKSPACES: [&str; 1] = ["web"];
const PERSONAS: [&str; 1] = ["builder"];

/// git, with no global or system config, in `dir`.
fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane whose root commits its project file, saying `settings`, and `policy.cedar`, holding
/// `rules` where there are any.
fn plane(settings: &str, rules: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("purlis.toml"), settings).expect("the project file");
    if let Some(rules) = rules {
        std::fs::write(dir.path().join("policy.cedar"), rules).expect("the rules");
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn loaded(settings: &str, rules: Option<&str>) -> Loaded {
    let dir = plane(settings, rules);
    levels::load(&Sources {
        root: dir.path(),
        workspaces: &WORKSPACES,
        personas: &PERSONAS,
        organisation: None,
        accepted: None,
    })
}

const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";

fn empty_world() -> World {
    World::from_json(serde_json::json!([])).expect("an empty world")
}

/// The host's context of a chat's action at the floor, attended, with `via` where the action's
/// context has it.
fn context(via: bool) -> serde_json::Value {
    let mut context = serde_json::json!({
        "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0
    });
    if via {
        context["via"] = "hook".into();
    }
    context
}

fn chat() -> Chat<'static> {
    Chat {
        workspace: Some("web"),
        persona: Some("builder"),
    }
}

fn merge() -> Request<'static> {
    Request {
        chat: "web/builder",
        of: chat(),
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: context(true),
    }
}

fn install(skill: &'static str) -> Request<'static> {
    Request {
        chat: "web/builder",
        of: chat(),
        action: "install",
        resource: ("Skill", skill),
        context: context(false),
    }
}

fn ids(decision: &Decision) -> Vec<&str> {
    decision.ids()
}

#[test]
fn a_merge_under_strict_asks_and_names_guards_merge_hold() {
    purlis_core::unsteered!();
    let decision = decide::decide(&loaded(STRICT, None), &merge(), &empty_world());
    assert_eq!(decision.outcome, Outcome::Ask);
    assert_eq!(decision.after_mode, Outcome::Ask);
    assert_eq!(decision.family, "guards.merge");
    assert_eq!(decision.mode, Mode::Enforce);
    assert_eq!(ids(&decision), ["guards-merge-hold"]);
    let rule = &decision.deciding[0];
    assert_eq!(rule.level, "project");
    assert!(
        rule.owner.starts_with("purlis default "),
        "the shipped default owns it: {rule:?}"
    );
    assert!(rule.line > 0, "its line is named: {rule:?}");
    assert!(
        decision
            .rules_said()
            .contains("`guards-merge-hold` (project, purlis default"),
        "{}",
        decision.rules_said()
    );
}

#[test]
fn a_merge_under_a_logged_family_is_allowed_and_would_have_blocked() {
    purlis_core::unsteered!();
    let settings = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"log\"\n";
    let decision = decide::decide(&loaded(settings, None), &merge(), &empty_world());
    assert_eq!(
        (decision.outcome, decision.mode, decision.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow)
    );
    assert!(decision.would_have_blocked());
}

#[test]
fn a_locked_family_is_enforced_whatever_its_mode_says() {
    purlis_core::unsteered!();
    let settings = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"off\"\n\n\
                    [policy.locks]\nfamilies = [\"guards.merge\"]\n";
    let decision = decide::decide(&loaded(settings, None), &merge(), &empty_world());
    assert_eq!(decision.lock.as_deref(), Some("this project"));
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Enforce, Outcome::Ask)
    );
}

#[test]
fn an_ask_with_no_permit_beneath_it_is_a_deny_naming_everything_else_off() {
    purlis_core::unsteered!();
    let settings = "[policy]\ntemplate = \"strict\"\n\n\
                    [policy.rules]\n\"policy-everything-else\" = \"off\"\n";
    let decision = decide::decide(&loaded(settings, None), &merge(), &empty_world());
    assert_eq!(decision.outcome, Outcome::Deny);
    assert_eq!(decision.after_mode, Outcome::Deny);
    assert_eq!(ids(&decision), ["policy-everything-else"]);
    assert!(decision.deciding[0].off);
    assert!(
        decision
            .rules_said()
            .starts_with("`policy-everything-else` (off)"),
        "{}",
        decision.rules_said()
    );
}

#[test]
fn no_permit_stays_a_deny_whatever_the_familys_mode() {
    purlis_core::unsteered!();
    // A mode acts on forbids only: turning a family off never turns deny-by-default into an
    // allow (Q319 C2).
    let settings = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"off\"\n\n\
                    [policy.rules]\n\"policy-everything-else\" = \"off\"\n";
    let decision = decide::decide(&loaded(settings, None), &merge(), &empty_world());
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Off, Outcome::Deny)
    );
}

/// A project that asks before installing `pdf`, and refuses `evil` outright.
const ASK_AND_DENY: &str = "// purlis-policy 1\n\
     @id(\"skills-ask-pdf\") @ask(\"you\")\n\
     forbid (principal, action == Action::\"install\", resource == Skill::\"pdf\");\n\
     @id(\"skills-ask-evil\") @ask(\"you\")\n\
     forbid (principal, action == Action::\"install\", resource == Skill::\"evil\");\n\
     @id(\"skills-no-evil\")\n\
     forbid (principal, action == Action::\"install\", resource == Skill::\"evil\");\n";

#[test]
fn an_ask_beats_an_allow() {
    purlis_core::unsteered!();
    let decision = decide::decide(
        &loaded(STRICT, Some(ASK_AND_DENY)),
        &install("pdf"),
        &empty_world(),
    );
    assert_eq!(decision.outcome, Outcome::Ask);
    // The shipped default asks of an unlisted skill too: both asks decide.
    assert!(ids(&decision).contains(&"skills-ask-pdf"), "{decision:?}");
}

#[test]
fn a_forbid_without_ask_beats_an_ask_at_the_same_decision() {
    purlis_core::unsteered!();
    let decision = decide::decide(
        &loaded(STRICT, Some(ASK_AND_DENY)),
        &install("evil"),
        &empty_world(),
    );
    assert_eq!(decision.outcome, Outcome::Deny);
    assert_eq!(ids(&decision), ["skills-no-evil"], "only the deny decides");
}

#[test]
fn a_narrower_permit_does_not_punch_through_an_ask() {
    purlis_core::unsteered!();
    let rules = "// purlis-policy 1\n\
         @id(\"skills-ask-pdf\") @ask(\"you\")\n\
         forbid (principal, action == Action::\"install\", resource == Skill::\"pdf\");\n\
         @id(\"skills-pdf-for-builders\")\n\
         permit (principal, action == Action::\"install\", resource == Skill::\"pdf\");\n";
    let decision = decide::decide(
        &loaded(STRICT, Some(rules)),
        &install("pdf"),
        &empty_world(),
    );
    assert_eq!(decision.outcome, Outcome::Ask);
    // The shipped default asks of an unlisted skill too: both asks decide.
    assert!(ids(&decision).contains(&"skills-ask-pdf"), "{decision:?}");
}

#[test]
fn a_forbid_inside_a_permit_decides_as_the_oracle_does() {
    purlis_core::unsteered!();
    // §14.6's probe: a permit over every skill, and a forbid on one skill inside it. The oracle
    // is the written action put straight to Cedar's authorizer (Q317.4), with no levels and no
    // modes between; `decide` must agree with it, and both deny.
    let rules = "// purlis-policy 1\n\
         @id(\"skills-every-one\")\n\
         permit (principal, action == Action::\"install\", resource);\n\
         @id(\"skills-not-evil\")\n\
         forbid (principal, action == Action::\"install\", resource == Skill::\"evil\");\n";
    let loaded = loaded(STRICT, Some(rules));
    for skill in ["evil", "pdf"] {
        let request = install(skill);
        let oracle = loaded.policy.decide(
            &Question {
                chat: request.chat,
                action: request.action,
                resource: request.resource,
                context: request.context.clone(),
            },
            &empty_world(),
        );
        let decision = decide::decide(&loaded, &request, &empty_world());
        assert_eq!(
            decision.outcome == Outcome::Allow,
            oracle.allows(),
            "{skill}: {decision:?} against {oracle:?}"
        );
    }
    let evil = decide::decide(&loaded, &install("evil"), &empty_world());
    assert_eq!(
        (evil.outcome, ids(&evil)),
        (Outcome::Deny, vec!["skills-not-evil"])
    );
}

#[test]
fn a_cedar_evaluation_error_is_a_deny_naming_the_rule() {
    purlis_core::unsteered!();
    // The chat is not in the world, so reading its workspace errs: the forbid that erred denies,
    // and is never skipped.
    let rules = "// purlis-policy 1\n\
         @id(\"skills-by-workspace\")\n\
         forbid (principal, action == Action::\"install\", resource)\n\
           when { principal.workspace == Workspace::\"web\" };\n";
    let settings = "[policy]\ntemplate = \"open\"\n";
    let decision = decide::decide(
        &loaded(settings, Some(rules)),
        &install("pdf"),
        &empty_world(),
    );
    assert_eq!(decision.outcome, Outcome::Deny);
    assert_eq!(
        decision.after_mode,
        Outcome::Deny,
        "an error is no forbid a mode may turn off"
    );
    assert_eq!(ids(&decision), ["skills-by-workspace"]);
}

#[test]
fn a_request_that_does_not_fit_the_schema_is_a_deny() {
    purlis_core::unsteered!();
    let mut request = merge();
    request.resource = ("Skill", "pdf");
    let decision = decide::decide(&loaded(STRICT, None), &request, &empty_world());
    assert_eq!(
        (decision.outcome, decision.after_mode),
        (Outcome::Deny, Outcome::Deny)
    );
    assert!(decision.unasked.is_some(), "{decision:?}");
}

#[test]
fn a_process_outside_any_plane_runs_the_shipped_strict_locally() {
    purlis_core::unsteered!();
    // No plane, no host: the shipped default under `strict`, decided in the asking process.
    let decision = decide::decide_locally(&merge());
    assert_eq!(
        (decision.outcome, decision.mode, decision.after_mode),
        (Outcome::Ask, Mode::Enforce, Outcome::Ask)
    );
    assert_eq!(ids(&decision), ["guards-merge-hold"]);
    // Strict enforces every family: a family `open` would turn off still decides here.
    let release = Request {
        action: "release",
        resource: ("Repo", "acme/web"),
        ..merge()
    };
    let decision = decide::decide_locally(&release);
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Enforce, Outcome::Ask)
    );
}

#[test]
fn a_decision_crosses_the_wire_whole() {
    purlis_core::unsteered!();
    let decision = decide::decide(&loaded(STRICT, None), &merge(), &empty_world());
    let wire = serde_json::to_string(&decision).expect("it is written");
    let read: Decision = serde_json::from_str(&wire).expect("it is read");
    assert_eq!(read, decision);
}
