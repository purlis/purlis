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

/// M75-04 round 2: `purlis change land` merges a change's requests, so the shipped default holds
/// it as it holds `gh pr merge`, never lets it through by the catch-all.
#[test]
fn a_land_under_strict_asks_and_names_guards_merge_hold() {
    purlis_core::unsteered!();
    let land = Request {
        action: "land",
        resource: ("Repo", "unknown"),
        ..merge()
    };
    let decision = decide::decide(&loaded(STRICT, None), &land, &empty_world());
    assert_eq!(decision.outcome, Outcome::Ask, "{decision:?}");
    assert_eq!(decision.after_mode, Outcome::Ask);
    assert_eq!(decision.family, "guards.merge");
    assert_eq!(ids(&decision), ["guards-merge-hold"]);
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

#[test]
fn the_replays_test_policy_host_answers_from_the_planes_committed_head() {
    purlis_core::unsteered!();
    use purlis_core::hookwire::{Answer, DecideAsk};
    let ask = DecideAsk {
        chat: 3,
        action: "merge".to_owned(),
        kind: "Branch".to_owned(),
        id: "acme/web#12".to_owned(),
        tool: "Bash".to_owned(),
        cwd: None,
    };
    let decided = |root: &Path| match decide::replays_host(root, &ask) {
        Answer::Decided(decision) => *decision,
        other => panic!("{other:?}"),
    };
    let dir = plane(STRICT, None);
    assert_eq!(decided(dir.path()).after_mode, Outcome::Ask);

    // A working file the chat wrote is never read: only what HEAD holds decides.
    let refusing = "// purlis-policy 1\n\
         @id(\"guards-merge-never\")\n\
         forbid (principal, action == Action::\"merge\", resource);\n";
    std::fs::write(dir.path().join("policy.cedar"), refusing).expect("the working file");
    assert_eq!(
        decided(dir.path()).after_mode,
        Outcome::Ask,
        "HEAD, not the working file"
    );

    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "refuse merges"]);
    let decision = decided(dir.path());
    assert_eq!(decision.after_mode, Outcome::Deny);
    assert_eq!(ids(&decision), ["guards-merge-never"]);
}

// ── the session host's decision for a chat (M75-04 round 2) ───────────────────────────────

mod for_a_chat {
    use super::*;
    use purlis_core::enforcement::Resolved;
    use purlis_core::policy::decide::ChatFacts;

    const OPEN: &str = "[policy]\ntemplate = \"open\"\n";

    fn facts(workspace: Option<&str>) -> ChatFacts {
        ChatFacts {
            workspace: workspace.map(str::to_owned),
            persona: None,
            sandboxed: false,
            attended: true,
        }
    }

    /// What the person accepted, or the machine locks, for `guards.merge` alone.
    fn floor(mode: &str) -> Resolved {
        Resolved::parse(&format!("guards.merge={mode}"))
    }

    fn merge_of(root: &Path, facts: Option<&ChatFacts>, floor: &Resolved) -> Option<Decision> {
        decide::for_chat(
            root,
            4,
            facts,
            floor,
            &decide::Sealed::unsealed(),
            "merge",
            ("Branch", "o/r#12"),
        )
    }

    #[test]
    fn only_what_an_enforcement_point_is_wired_for_is_asked_about() {
        purlis_core::unsteered!();
        assert!(decide::is_asked_by_an_enforcement_point("merge", "Branch"));
        assert!(decide::is_asked_by_an_enforcement_point("land", "Repo"));
        for (action, kind) in [
            ("read", "File"),
            ("merge", "Repo"),
            ("land", "Branch"),
            ("push", "Branch"),
            ("", ""),
        ] {
            assert!(
                !decide::is_asked_by_an_enforcement_point(action, kind),
                "{action} on {kind}"
            );
        }
    }

    #[test]
    fn a_chat_the_host_has_not_open_is_decided_nothing_for() {
        purlis_core::unsteered!();
        let dir = plane(STRICT, None);
        assert_eq!(merge_of(dir.path(), None, &floor("enforce")), None);
    }

    #[test]
    fn the_asking_chats_workspace_chooses_the_mode() {
        purlis_core::unsteered!();
        let dir = plane(
            "[policy]\ntemplate = \"open\"\n\n[policy.workspace.web]\ntemplate = \"strict\"\n",
            None,
        );
        std::fs::create_dir_all(dir.path().join("workspaces/web")).expect("a workspace");
        let loose = floor("off");
        let in_web = merge_of(dir.path(), Some(&facts(Some("web"))), &loose).expect("decided");
        assert_eq!(in_web.after_mode, Outcome::Ask, "{in_web:?}");
        let elsewhere = merge_of(dir.path(), Some(&facts(None)), &loose).expect("decided");
        assert_eq!(elsewhere.after_mode, Outcome::Allow, "{elsewhere:?}");
        assert!(elsewhere.would_have_blocked(), "{elsewhere:?}");
    }

    #[test]
    fn a_head_that_loosens_the_merge_hold_does_not_loosen_what_the_person_accepted() {
        purlis_core::unsteered!();
        // Each way a commit reaching `HEAD` can switch the hold off.
        for settings in [
            OPEN,
            "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"off\"\n",
            "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"guards-merge-hold\" = \"off\"\n",
        ] {
            let dir = plane(settings, None);
            let facts = facts(None);
            let loosened = merge_of(dir.path(), Some(&facts), &floor("log")).expect("decided");
            assert_eq!(
                loosened.after_mode,
                Outcome::Allow,
                "{settings}: {loosened:?}"
            );
            let held = merge_of(dir.path(), Some(&facts), &floor("enforce")).expect("decided");
            assert_eq!(held.after_mode, Outcome::Ask, "{settings}: {held:?}");
            assert_eq!(held.mode, Mode::Enforce, "{settings}: {held:?}");
            assert_eq!(held.ids(), ["guards-merge-hold"], "{settings}");
        }
    }

    #[test]
    fn a_land_is_held_to_the_floor_as_a_merge_is() {
        purlis_core::unsteered!();
        let dir = plane(OPEN, None);
        let land = decide::for_chat(
            dir.path(),
            4,
            Some(&facts(None)),
            &floor("enforce"),
            &decide::Sealed::unsealed(),
            "land",
            ("Repo", "unknown"),
        )
        .expect("decided");
        assert_eq!(land.after_mode, Outcome::Ask, "{land:?}");
    }

    #[test]
    fn a_family_the_project_locks_is_enforced_whatever_its_template_says() {
        purlis_core::unsteered!();
        let dir = plane(
            "[policy]\ntemplate = \"open\"\n\n[policy.locks]\nfamilies = [\"guards.merge\"]\n",
            None,
        );
        let locked = merge_of(dir.path(), Some(&facts(None)), &floor("off")).expect("decided");
        assert_eq!(locked.after_mode, Outcome::Ask, "{locked:?}");
        assert_eq!(locked.mode, Mode::Enforce);
        assert_eq!(locked.lock.as_deref(), Some("this project"), "{locked:?}");
    }
}

// ── M75 slice 10 (#7019): modes and rule modes, applied after Cedar decides (spec §6.4) ──────

/// A request of `action` on `kind` `id`, whose context is `context`.
fn request<'a>(
    action: &'a str,
    resource: (&'a str, &'a str),
    context: serde_json::Value,
) -> Request<'a> {
    Request {
        chat: "web/builder",
        of: chat(),
        action,
        resource,
        context,
    }
}

/// The context of a `connect` or a `dial`, which names the port and the way it came.
fn network_context() -> serde_json::Value {
    let mut context = context(false);
    context["port"] = 443.into();
    context["via"] = "proxy".into();
    context
}

/// An `exec` of `command`, which the shipped default allows.
fn exec(command: &'static str) -> Request<'static> {
    request("exec", ("Command", command), context(false))
}

#[test]
fn a_rule_mode_beats_its_family_mode() {
    purlis_core::unsteered!();
    // The rule's own `log` beats a family that enforces.
    let log = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"enforce\"\n\n\
               [policy.rules]\n\"guards-merge-hold\" = \"log\"\n";
    let decision = decide::decide(&loaded(log, None), &merge(), &empty_world());
    assert_eq!(
        (decision.outcome, decision.mode, decision.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow)
    );
    assert!(decision.would_have_blocked());

    // The rule's own `enforce` beats a family that logs.
    let enforce = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"log\"\n\n\
                   [policy.rules]\n\"guards-merge-hold\" = \"enforce\"\n";
    let decision = decide::decide(&loaded(enforce, None), &merge(), &empty_world());
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Enforce, Outcome::Ask)
    );

    // One rule's `log` never loosens the forbid beside it that has no mode of its own: the
    // family enforces that one, so the deny stands.
    let rules = "// purlis-policy 1\n\
         @id(\"skills-no-evil\")\n\
         forbid (principal, action == Action::\"install\", resource == Skill::\"evil\");\n\
         @id(\"skills-no-evil-either\")\n\
         forbid (principal, action == Action::\"install\", resource == Skill::\"evil\");\n";
    let one = "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"skills-no-evil\" = \"log\"\n";
    let decision = decide::decide(&loaded(one, Some(rules)), &install("evil"), &empty_world());
    assert_eq!(decision.outcome, Outcome::Deny);
    // Only the forbid still enforced decides; the one at `log` is lifted.
    assert_eq!(ids(&decision), ["skills-no-evil-either"], "{decision:?}");
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Enforce, Outcome::Deny)
    );
    // Both plain forbids lifted: the shipped ask on an unlisted skill still applies and is
    // enforced, so the person is asked rather than the deny turning into an allow.
    let both = format!("{one}\"skills-no-evil-either\" = \"log\"\n");
    let decision = decide::decide(
        &loaded(&both, Some(rules)),
        &install("evil"),
        &empty_world(),
    );
    assert_eq!(decision.after_mode, Outcome::Ask, "{decision:?}");
    assert_eq!(ids(&decision), ["skills-unlisted"]);
}

#[test]
fn a_lock_beats_a_rule_mode_and_a_family_mode() {
    purlis_core::unsteered!();
    let settings = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"off\"\n\n\
                    [policy.locks]\nfamilies = [\"guards.merge\"]\n\n\
                    [policy.rules]\n\"guards-merge-hold\" = \"log\"\n";
    let decision = decide::decide(&loaded(settings, None), &merge(), &empty_world());
    assert_eq!(decision.lock.as_deref(), Some("this project"));
    assert_eq!(
        (decision.mode, decision.after_mode),
        (Mode::Enforce, Outcome::Ask)
    );
}

#[test]
fn everything_else_off_denies_what_no_permit_allows_and_names_it() {
    purlis_core::unsteered!();
    let allowed = decide::decide(&loaded(STRICT, None), &exec("ls"), &empty_world());
    assert_eq!(allowed.outcome, Outcome::Allow, "{allowed:?}");

    let off =
        "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"policy-everything-else\" = \"off\"\n";
    let decision = decide::decide(&loaded(off, None), &exec("ls"), &empty_world());
    assert_eq!(
        (decision.outcome, decision.after_mode),
        (Outcome::Deny, Outcome::Deny)
    );
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
fn a_family_turned_off_never_denies_under_deny_by_default() {
    purlis_core::unsteered!();
    // Deny-by-default, a permit for every program, and a forbid on one.
    let rules = "// purlis-policy 1\n\
         @id(\"processes-allow-all\")\n\
         permit (principal, action == Action::\"exec\", resource);\n\
         @id(\"processes-no-rm\")\n\
         forbid (principal, action == Action::\"exec\", resource == Command::\"rm\");\n";
    let settings = "[policy]\ntemplate = \"strict\"\nprocesses = \"off\"\n\n\
                    [policy.rules]\n\"policy-everything-else\" = \"off\"\n";
    let loaded = loaded(settings, Some(rules));
    // The family's forbid does not run, so it never denies; the permit under it counts.
    let rm = decide::decide(&loaded, &exec("rm"), &empty_world());
    assert_eq!(
        (rm.outcome, rm.mode, rm.after_mode),
        (Outcome::Deny, Mode::Off, Outcome::Allow)
    );
    let ls = decide::decide(&loaded, &exec("ls"), &empty_world());
    assert_eq!(ls.after_mode, Outcome::Allow, "{ls:?}");
    assert_eq!(ids(&ls), ["processes-allow-all"]);
    // An action no permit covers is still deny-by-default's, whatever the family.
    let other = decide::decide(&loaded, &install("pdf"), &empty_world());
    assert_eq!(other.after_mode, Outcome::Deny);
}

#[test]
fn a_family_mode_leaves_that_familys_permits_counting() {
    purlis_core::unsteered!();
    let rules = "// purlis-policy 1\n\
         @id(\"processes-allow-ls\")\n\
         permit (principal, action == Action::\"exec\", resource == Command::\"ls\");\n";
    for mode in ["off", "log", "enforce"] {
        let settings = format!(
            "[policy]\ntemplate = \"strict\"\nprocesses = \"{mode}\"\n\n\
             [policy.rules]\n\"policy-everything-else\" = \"off\"\n"
        );
        let loaded = loaded(&settings, Some(rules));
        let ls = decide::decide(&loaded, &exec("ls"), &empty_world());
        assert_eq!(
            (ls.outcome, ids(&ls)),
            (Outcome::Allow, vec!["processes-allow-ls"]),
            "{mode}"
        );
        let cat = decide::decide(&loaded, &exec("cat"), &empty_world());
        assert_eq!(
            cat.after_mode,
            Outcome::Deny,
            "{mode}: no permit is no mode's"
        );
    }
    // A permit stops counting only where `[policy.rules]` names it `off`.
    let off = "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\
               \"policy-everything-else\" = \"off\"\n\"processes-allow-ls\" = \"off\"\n";
    let ls = decide::decide(&loaded(off, Some(rules)), &exec("ls"), &empty_world());
    assert_eq!(ls.after_mode, Outcome::Deny);
}

/// A project that asks of one host and one socket, forbids another host, and asks of a write.
const ASKS_AND_A_FORBID: &str = "// purlis-policy 1\n\
     @id(\"network-no-evil\")\n\
     forbid (principal, action == Action::\"connect\", resource == Host::\"evil.example\");\n\
     @id(\"network-ask-socket\") @ask(\"you\")\n\
     forbid (principal, action == Action::\"dial\", resource == Socket::\"/run/x.sock\");\n\
     @id(\"network-no-socket\")\n\
     forbid (principal, action == Action::\"dial\", resource == Socket::\"/run/y.sock\");\n\
     @id(\"sandbox-ask-write\") @ask(\"you\")\n\
     forbid (principal, action == Action::\"write\", resource == Path::\"/srv/x\");\n\
     @id(\"sandbox-no-write\")\n\
     forbid (principal, action == Action::\"write\", resource == Path::\"/srv/y\");\n";

/// `asks` logs, `network` enforces.
const ASKS_LOG: &str = "[policy]\ntemplate = \"strict\"\nasks = \"log\"\nnetwork = \"enforce\"\n";

#[test]
fn an_ask_on_connect_takes_the_asks_mode_and_a_plain_forbid_the_network_mode() {
    purlis_core::unsteered!();
    let loaded = loaded(ASKS_LOG, Some(ASKS_AND_A_FORBID));
    let ask = decide::decide(
        &loaded,
        &request("connect", ("Host", "other.example"), network_context()),
        &empty_world(),
    );
    assert_eq!(ids(&ask), ["network-unknown-host"]);
    assert_eq!(
        (ask.family.as_str(), ask.mode_family.as_str()),
        ("network", "asks")
    );
    assert_eq!(
        (ask.outcome, ask.mode, ask.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow)
    );

    let plain = decide::decide(
        &loaded,
        &request("connect", ("Host", "evil.example"), network_context()),
        &empty_world(),
    );
    assert_eq!(ids(&plain), ["network-no-evil"]);
    assert_eq!(plain.mode_family, "network");
    assert_eq!(
        (plain.outcome, plain.mode, plain.after_mode),
        (Outcome::Deny, Mode::Enforce, Outcome::Deny)
    );

    // The other way round: the plain forbid logs, the ask enforces.
    let flipped = "[policy]\ntemplate = \"strict\"\nasks = \"enforce\"\nnetwork = \"log\"\n";
    let loaded = self::loaded(flipped, Some(ASKS_AND_A_FORBID));
    let ask = decide::decide(
        &loaded,
        &request("connect", ("Host", "other.example"), network_context()),
        &empty_world(),
    );
    assert_eq!((ask.mode, ask.after_mode), (Mode::Enforce, Outcome::Ask));
    let plain = decide::decide(
        &loaded,
        &request("connect", ("Host", "evil.example"), network_context()),
        &empty_world(),
    );
    // The plain forbid logs, but the ask beside it is enforced: it is not hidden (round 2, B1).
    assert_eq!(
        (plain.outcome, plain.after_mode),
        (Outcome::Deny, Outcome::Ask)
    );
    assert_eq!(ids(&plain), ["network-unknown-host"]);

    // A lock on `asks` binds an ask on connect, though the action's family is `network`.
    let locked = format!("{ASKS_LOG}\n[policy.locks]\nfamilies = [\"asks\"]\n");
    let ask = decide::decide(
        &self::loaded(&locked, Some(ASKS_AND_A_FORBID)),
        &request("connect", ("Host", "other.example"), network_context()),
        &empty_world(),
    );
    assert_eq!(ask.lock.as_deref(), Some("this project"));
    assert_eq!((ask.mode, ask.after_mode), (Mode::Enforce, Outcome::Ask));
}

#[test]
fn an_ask_on_a_dial_takes_the_asks_mode() {
    purlis_core::unsteered!();
    let loaded = loaded(ASKS_LOG, Some(ASKS_AND_A_FORBID));
    let ask = decide::decide(
        &loaded,
        &request("dial", ("Socket", "/run/x.sock"), network_context()),
        &empty_world(),
    );
    assert_eq!(ids(&ask), ["network-ask-socket"]);
    assert_eq!(
        (ask.family.as_str(), ask.mode_family.as_str()),
        ("network", "asks")
    );
    assert_eq!(
        (ask.outcome, ask.mode, ask.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow)
    );
    let plain = decide::decide(
        &loaded,
        &request("dial", ("Socket", "/run/y.sock"), network_context()),
        &empty_world(),
    );
    assert_eq!(
        (plain.mode_family.as_str(), plain.mode, plain.after_mode),
        ("network", Mode::Enforce, Outcome::Deny)
    );
}

#[test]
fn an_ask_on_a_path_write_takes_the_asks_mode() {
    purlis_core::unsteered!();
    let loaded = loaded(ASKS_LOG, Some(ASKS_AND_A_FORBID));
    let ask = decide::decide(
        &loaded,
        &request("write", ("Path", "/srv/x"), context(false)),
        &empty_world(),
    );
    assert_eq!(ids(&ask), ["sandbox-ask-write"]);
    assert_eq!(
        (ask.family.as_str(), ask.mode_family.as_str()),
        ("sandbox", "asks")
    );
    assert_eq!(
        (ask.outcome, ask.mode, ask.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow)
    );
    // A plain forbid on a write takes the `sandbox` family's, which a template gives.
    let plain = decide::decide(
        &loaded,
        &request("write", ("Path", "/srv/y"), context(false)),
        &empty_world(),
    );
    assert_eq!(plain.mode_family, "sandbox");
    assert_eq!(
        (plain.mode, plain.after_mode),
        (Mode::Enforce, Outcome::Deny)
    );
}

// ── M75-10 round 2: every forbid has its own mode, and a lifted forbid invents no allow ─────

use purlis_core::policy::levels::Organisation;

/// The decision's loaded policy with an Organisation's `rules` and `settings` on it.
fn loaded_under(
    settings: &str,
    rules: Option<&str>,
    org_rules: Option<&str>,
    org_settings: Option<&str>,
) -> Loaded {
    let dir = plane(settings, rules);
    levels::load(&Sources {
        root: dir.path(),
        workspaces: &WORKSPACES,
        personas: &PERSONAS,
        organisation: Some(Organisation {
            folder: "/etc/purlis",
            owner: "Platform team",
            rules: org_rules,
            settings: org_settings,
            json: None,
            failed: None,
        }),
        accepted: None,
    })
}

/// The Organisation's ask on one host, and a project forbid on every connect.
const ORG_ASK: &str = "// purlis-policy 1\n\
     @id(\"network-org-ask\") @ask(\"you\")\n\
     forbid (principal, action == Action::\"connect\", resource == Host::\"x.example\");\n";
const SHADOW: &str = "// purlis-policy 1\n\
     @id(\"network-shadow\")\n\
     forbid (principal, action == Action::\"connect\", resource);\n";

fn connect_x() -> Request<'static> {
    request("connect", ("Host", "x.example"), network_context())
}

#[test]
fn a_plain_forbid_that_logs_never_hides_an_organisation_ask_that_enforces() {
    purlis_core::unsteered!();
    // The project's plain forbid, at `log`, applies beside the Organisation's ask.
    let shadowed =
        "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"network-shadow\" = \"log\"\n";
    let loaded = loaded_under(shadowed, Some(SHADOW), Some(ORG_ASK), None);
    let decision = decide::decide(&loaded, &connect_x(), &empty_world());
    assert_eq!(
        decision.outcome,
        Outcome::Deny,
        "Cedar: the plain forbid denies"
    );
    assert_eq!(decision.after_mode, Outcome::Ask, "{decision:?}");
    assert!(ids(&decision).contains(&"network-org-ask"), "{decision:?}");
    assert!(!ids(&decision).contains(&"network-shadow"), "{decision:?}");

    // The network family and the asks family both at `log`: the Organisation's forbid is
    // still enforced.
    let logged = "[policy]\ntemplate = \"strict\"\nnetwork = \"log\"\nasks = \"log\"\n";
    let loaded = loaded_under(logged, Some(SHADOW), Some(ORG_ASK), None);
    let decision = decide::decide(&loaded, &connect_x(), &empty_world());
    assert_eq!(decision.after_mode, Outcome::Ask, "{decision:?}");
    assert!(ids(&decision).contains(&"network-org-ask"), "{decision:?}");
}

#[test]
fn a_lifted_forbid_never_invents_an_allow_where_no_permit_applies() {
    purlis_core::unsteered!();
    let rules = "// purlis-policy 1\n\
         @id(\"processes-allow-ls\")\n\
         permit (principal, action == Action::\"exec\", resource == Command::\"ls\");\n\
         @id(\"processes-no-rm\")\n\
         forbid (principal, action == Action::\"exec\", resource == Command::\"rm\");\n";
    for mode in ["off", "log"] {
        let settings = format!(
            "[policy]\ntemplate = \"strict\"\nprocesses = \"{mode}\"\n\n\
             [policy.rules]\n\"policy-everything-else\" = \"off\"\n"
        );
        let loaded = loaded(&settings, Some(rules));
        let rm = decide::decide(&loaded, &exec("rm"), &empty_world());
        assert_eq!(rm.after_mode, Outcome::Deny, "{mode}: {rm:?}");
        assert_eq!(ids(&rm), ["policy-everything-else"], "{mode}");
        // The deny is no `log`'s doing, so `policy why` never reads one beside it.
        assert_ne!(rm.mode, Mode::Log, "{mode}: {rm:?}");
        let cat = decide::decide(&loaded, &exec("cat"), &empty_world());
        assert_eq!(cat.after_mode, Outcome::Deny, "{mode}");
        let ls = decide::decide(&loaded, &exec("ls"), &empty_world());
        assert_eq!(ls.after_mode, Outcome::Allow, "{mode}");
    }
}

#[test]
fn a_lock_on_asks_holds_an_ask_rule_against_off_and_log() {
    purlis_core::unsteered!();
    let org = "[policy.locks]\nfamilies = [\"asks\"]\n";
    for mode in ["off", "log"] {
        let settings = format!(
            "[policy]\ntemplate = \"strict\"\nasks = \"log\"\n\n\
             [policy.rules]\n\"network-unknown-host\" = \"{mode}\"\n"
        );
        let loaded = loaded_under(&settings, None, None, Some(org));
        assert!(
            loaded.policy.rule("network-unknown-host").is_some(),
            "{mode}: the rule is not removed"
        );
        let decision = decide::decide(
            &loaded,
            &request("connect", ("Host", "other.example"), network_context()),
            &empty_world(),
        );
        assert_eq!(decision.after_mode, Outcome::Ask, "{mode}: {decision:?}");
        assert_eq!(decision.lock.as_deref(), Some("Platform team"), "{mode}");
    }
}

#[test]
fn a_rule_that_cannot_log_enforces_at_decide_time_whatever_it_names() {
    purlis_core::unsteered!();
    // A forbid on every action is no vaults rule by what it says, so the loader keeps its
    // `log`; deciding a `use`, the family that cannot log enforces it all the same.
    let rules = "// purlis-policy 1\n\
         @id(\"vaults-everything\")\n\
         forbid (principal, action, resource);\n";
    let settings = "[policy]\ntemplate = \"strict\"\n\n[policy.rules]\n\"vaults-everything\" = \"log\"\n\"vaults-use\" = \"off\"\n\"vaults-other-persona\" = \"off\"\n";
    let request = request("use", ("Secret", "vault/key"), {
        let mut context = context(false);
        context["program"] = "ls".into();
        context["declared"] = true.into();
        context
    });
    let decision = decide::decide(&loaded(settings, Some(rules)), &request, &empty_world());
    assert_eq!(decision.after_mode, Outcome::Deny, "{decision:?}");
    assert_eq!(decision.mode, Mode::Enforce);
    assert_eq!(ids(&decision), ["vaults-everything"], "{decision:?}");
}

// ── M75-10 round 3: a floor restores a deny a mode lowered to an ask ────────────────────────

/// `network` at `log` (the plain forbid lifted) and `asks` enforced (the shipped ask stands).
const PLAIN_LIFTED_ASK_ENFORCED: &str =
    "[policy]\ntemplate = \"strict\"\nnetwork = \"log\"\nasks = \"enforce\"\n";

fn unknown_host() -> Request<'static> {
    request("connect", ("Host", "other.example"), network_context())
}

#[test]
fn a_floor_that_enforces_the_family_puts_back_a_deny_a_mode_lowered_to_an_ask() {
    purlis_core::unsteered!();
    use purlis_core::enforcement::Resolved;
    let loaded = loaded(PLAIN_LIFTED_ASK_ENFORCED, Some(SHADOW));
    let decided = decide::decide(&loaded, &unknown_host(), &empty_world());
    assert_eq!(
        (decided.outcome, decided.after_mode),
        (Outcome::Deny, Outcome::Ask),
        "{decided:?}"
    );
    assert_eq!(ids(&decided), ["network-unknown-host"]);

    let held = decided.clone().held_to(&Resolved::parse("network=enforce"));
    assert_eq!(
        (held.outcome, held.after_mode, held.mode),
        (Outcome::Deny, Outcome::Deny, Mode::Enforce),
        "{held:?}"
    );
    // The plain forbids that decide it are named, not the ask they stood beside.
    assert_eq!(ids(&held), ["network-shadow"], "{held:?}");

    // A floor that does not enforce the family holds nothing.
    for mode in ["log", "off"] {
        let loose = decided
            .clone()
            .held_to(&Resolved::parse(&format!("network={mode}")));
        assert_eq!(loose.after_mode, Outcome::Ask, "{mode}: {loose:?}");
    }
}

#[test]
fn a_known_host_and_an_unknown_one_are_held_alike_by_a_floor() {
    purlis_core::unsteered!();
    use purlis_core::enforcement::Resolved;
    let settings =
        format!("{PLAIN_LIFTED_ASK_ENFORCED}\n[policy.sets]\nknown-hosts = [\"github.com\"]\n");
    let loaded = loaded(&settings, Some(SHADOW));
    let floor = Resolved::parse("network=enforce");
    let known = request("connect", ("Host", "github.com"), network_context());
    for request in [known, unknown_host()] {
        let held = decide::decide(&loaded, &request, &empty_world()).held_to(&floor);
        assert_eq!(held.after_mode, Outcome::Deny, "{held:?}");
    }
}

#[test]
fn an_enforced_plain_forbid_beside_an_enforced_ask_denies() {
    purlis_core::unsteered!();
    let enforced = "[policy]\ntemplate = \"strict\"\nnetwork = \"enforce\"\nasks = \"enforce\"\n";
    let decision = decide::decide(
        &loaded(enforced, Some(SHADOW)),
        &unknown_host(),
        &empty_world(),
    );
    assert_eq!(
        (decision.outcome, decision.after_mode),
        (Outcome::Deny, Outcome::Deny),
        "{decision:?}"
    );
    assert_eq!(ids(&decision), ["network-shadow"], "{decision:?}");
}

#[test]
fn a_project_cannot_replace_an_organisation_file_rule_and_is_told_so() {
    purlis_core::unsteered!();
    // The same `@id` as the Organisation's ask, as a permit that would let every connect through.
    let replacement = "// purlis-policy 1\n\
         @id(\"network-org-ask\")\n\
         permit (principal, action == Action::\"connect\", resource);\n";
    let loaded = loaded_under(STRICT, Some(replacement), Some(ORG_ASK), None);
    let decision = decide::decide(&loaded, &connect_x(), &empty_world());
    assert_eq!(decision.after_mode, Outcome::Ask, "{decision:?}");
    assert!(ids(&decision).contains(&"network-org-ask"), "{decision:?}");
    let said = loaded
        .notices
        .iter()
        .find(|n| n.what.contains("can't replace"))
        .expect("a Notice says the rule is refused");
    assert!(
        said.what.contains("an Organisation rule"),
        "an Organisation-file forbid has no [policy.rules] entry of its own to cite: {}",
        said.what
    );
}

mod a_floor_holds_a_chats_merge {
    use super::*;
    use purlis_core::enforcement::Resolved;
    use purlis_core::policy::decide::ChatFacts;

    /// A plain forbid on a merge, beside the shipped `guards-merge-hold` ask.
    const PLAIN_MERGE: &str = "// purlis-policy 1\n\
         @id(\"guards-merge-plain\")\n\
         forbid (principal, action == Action::\"merge\", resource);\n";

    #[test]
    fn a_plain_forbid_that_logs_is_put_back_beside_the_hold_that_asks() {
        purlis_core::unsteered!();
        let settings = "[policy]\ntemplate = \"strict\"\n\n\
                        [policy.rules]\n\"guards-merge-plain\" = \"log\"\n";
        let dir = plane(settings, Some(PLAIN_MERGE));
        let facts = ChatFacts {
            workspace: None,
            persona: None,
            sandboxed: false,
            attended: true,
        };
        let merge = |floor: &str| {
            decide::for_chat(
                dir.path(),
                4,
                Some(&facts),
                &Resolved::parse(floor),
                &decide::Sealed::unsealed(),
                "merge",
                ("Branch", "o/r#12"),
            )
            .expect("decided")
        };
        let held = merge("guards.merge=enforce");
        assert_eq!(
            (held.outcome, held.after_mode, held.mode),
            (Outcome::Deny, Outcome::Deny, Mode::Enforce),
            "{held:?}"
        );
        assert_eq!(ids(&held), ["guards-merge-plain"], "{held:?}");
        // Without the floor the project's own `log` stands, and the hold asks.
        let loose = merge("guards.merge=log");
        assert_eq!(loose.after_mode, Outcome::Ask, "{loose:?}");
    }
}
