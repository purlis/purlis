// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Mode, runners, consents, processes and audit retention** (M75 slice 36, #7045; spec §3.2,
//! §3.3 and §5.1; Q320e R6-1).
//!
//! The `mode` family judges the permission mode a chat runs in, by `context.harness_mode` and
//! `context.attended`, which the session host fills from its own record of the chat's start and
//! never from a word of the chat's. `runners` holds a chat's `offload` and `forward`, `consents`
//! is the window's alone, and `processes` holds what the floor and the full sandbox add beyond
//! slice 11's escape denies. Audit retention is three `audit.*` limits an Organisation and a
//! project may set and lock, with the minimums of spec §5.1, and no policy turns the record off.
//!
//! Every test that proves a refusal runs under `strict` explicitly (ADR 0125).

use std::path::Path;

use purlis_core::attendance::{Begun, ClaudeCodeMode, Fact};
use purlis_core::enforcement::{Mode, Resolved};
use purlis_core::harness::Harness;
use purlis_core::policy::World;
use purlis_core::policy::decide::{self, ChatFacts, Decision, Outcome, Request};
use purlis_core::policy::levels::{self, Accepted, Chat, Loaded, Organisation, Sources};
use purlis_core::policy::limits::{Direction, Shipped, Verdict};

const WORKSPACES: [&str; 1] = ["web"];
const PERSONAS: [&str; 1] = ["builder"];
const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";

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

/// A plane whose root commits its project file, saying `settings`.
fn plane(settings: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("purlis.toml"), settings).expect("the project file");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn sources<'a>(
    root: &'a Path,
    organisation: Option<Organisation<'a>>,
    accepted: Option<&'a str>,
) -> Sources<'a> {
    Sources {
        root,
        workspaces: &WORKSPACES,
        personas: &PERSONAS,
        organisation,
        accepted: accepted.map(|settings| Accepted {
            settings: Some(settings),
            rules: None,
        }),
    }
}

fn loaded(settings: &str) -> Loaded {
    let dir = plane(settings);
    levels::load(&sources(dir.path(), None, None))
}

fn organisation(settings: &str) -> Organisation<'_> {
    Organisation {
        folder: "/etc/purlis",
        owner: "Platform team",
        rules: None,
        settings: Some(settings),
        json: None,
        failed: None,
    }
}

fn chat() -> Chat<'static> {
    Chat {
        workspace: Some("web"),
        persona: Some("builder"),
    }
}

fn empty_world() -> World {
    World::from_json(serde_json::json!([])).expect("an empty world")
}

/// The context of a chat's action at `sandbox`, attended or not, started in `harness_mode`.
fn context(sandbox: &str, attended: bool, harness_mode: &str) -> serde_json::Value {
    serde_json::json!({
        "sandbox": sandbox, "attended": attended, "harness_mode": harness_mode, "depth": 0
    })
}

fn decided(
    loaded: &Loaded,
    action: &'static str,
    resource: (&'static str, &'static str),
    context: serde_json::Value,
) -> Decision {
    decide::decide(
        loaded,
        &Request {
            chat: "web/builder",
            of: chat(),
            action,
            resource,
            context,
        },
        &empty_world(),
    )
}

/// The rule that decided, named as a refusal names it: its id, level and owner.
fn assert_names(decision: &Decision, id: &str) {
    assert_eq!(decision.ids(), [id], "{decision:#?}");
    let rule = &decision.deciding[0];
    assert_eq!(rule.level, "project", "{rule:?}");
    assert!(
        rule.owner.starts_with("purlis default "),
        "the shipped default owns it: {rule:?}"
    );
    assert!(rule.line > 0, "its line is named: {rule:?}");
    assert!(
        decision
            .rules_said()
            .contains(&format!("`{id}` (project, purlis default")),
        "{}",
        decision.rules_said()
    );
}

// ── mode ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_dispatched_task_above_the_mode_rule_is_refused_and_named() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    let enter = |attended, mode| {
        decided(
            &strict,
            "enter",
            ("Harness", "claude-code"),
            context("floor", attended, mode),
        )
    };
    // A task is capped at auto (Dispatch #42): its prompts off is above the mode rule.
    let refused = enter(false, "bypassPermissions");
    assert_eq!(refused.outcome, Outcome::Deny);
    assert_eq!(refused.after_mode, Outcome::Deny);
    assert_eq!(refused.family, "mode");
    assert_eq!(refused.mode, Mode::Enforce);
    assert_names(&refused, "mode-task-above-auto");
    // Not above it: the person's own chat in bypass (Mode #1), and a task at auto or default.
    assert_eq!(enter(true, "bypassPermissions").outcome, Outcome::Allow);
    assert_eq!(enter(false, "auto").outcome, Outcome::Allow);
    assert_eq!(enter(false, "default").outcome, Outcome::Allow);
}

/// The chat's start as the host holds it: a task of Claude Code started on `command`.
fn a_task_started_on(command: &[&str]) -> Fact {
    let command: Vec<String> = command.iter().map(|word| (*word).to_owned()).collect();
    Fact::at_start(Begun::Task, Some(Harness::ClaudeCode), &command)
}

/// What the host records of the chat `fact` is the attendance of.
fn facts_of(fact: Fact) -> ChatFacts {
    ChatFacts {
        workspace: Some("web".to_owned()),
        persona: Some("builder".to_owned()),
        sandboxed: true,
        level: None,
        attended: fact.attendance() == purlis_core::dispatchunattended::Attendance::Attended,
        harness_mode: fact.start_mode(),
    }
}

fn enter_for(root: &Path, facts: &ChatFacts) -> Decision {
    decide::for_chat(
        root,
        4,
        Some(facts),
        &Resolved::parse(""),
        &decide::Sealed::unsealed(),
        "enter",
        ("Harness", "claude-code"),
    )
    .expect("a chat the host has open")
}

#[test]
fn harness_mode_comes_from_the_hosts_start_record_never_the_chat() {
    purlis_core::unsteered!();
    let dir = plane(STRICT);

    // The host started a task with its prompts off: that is the mode judged.
    let mut task = a_task_started_on(&["claude", "--permission-mode", "bypassPermissions"]);
    assert_eq!(task.start_mode(), Some(ClaudeCodeMode::BypassPermissions));
    let refused = enter_for(dir.path(), &facts_of(task));
    assert_eq!(refused.after_mode, Outcome::Deny, "{refused:#?}");
    assert_names(&refused, "mode-task-above-auto");

    // What the chat's hook later says of its mode moves nothing: the start record's is judged.
    task.hear(Some(ClaudeCodeMode::Default));
    assert_eq!(task.start_mode(), Some(ClaudeCodeMode::BypassPermissions));
    assert_eq!(
        enter_for(dir.path(), &facts_of(task)).after_mode,
        Outcome::Deny,
        "a chat cannot lower the mode it is judged at"
    );

    // And a chat started at its default that says bypass is judged at the default it started in.
    let mut calm = a_task_started_on(&["claude"]);
    calm.hear(Some(ClaudeCodeMode::BypassPermissions));
    assert_eq!(calm.start_mode(), Some(ClaudeCodeMode::Default));
    assert_eq!(
        enter_for(dir.path(), &facts_of(calm)).after_mode,
        Outcome::Allow,
        "a chat cannot raise it either"
    );

    // A start the host kept no mode for (a harness whose mode purlis does not read, a relaunch)
    // reads as `unknown`, which `mode-task-above-auto` allows (a known gap, slice 37).
    let unread = Fact::at_start(Begun::Task, Some(Harness::Codex), &[]);
    assert_eq!(unread.start_mode(), None);
    assert_eq!(decide::harness_mode_word(unread.start_mode()), "unknown");
    assert_eq!(
        decide::harness_mode_word(Some(ClaudeCodeMode::Auto)),
        "auto"
    );

    // The ask a hook sends has no field to say it in: a line that names one is refused whole.
    let line = serde_json::json!({
        "chat": 4, "action": "enter", "kind": "Harness", "id": "claude-code", "tool": "Bash",
        "harness_mode": "default"
    });
    assert!(
        serde_json::from_value::<purlis_core::hookwire::DecideAsk>(line).is_err(),
        "no word of a chat's line reaches context.harness_mode"
    );
}

#[test]
fn a_mode_refusal_names_its_id_level_and_owner() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    let refused = decided(
        &strict,
        "enter",
        ("Harness", "claude-code"),
        context("full", false, "bypassPermissions"),
    );
    assert_eq!(refused.outcome, Outcome::Deny);
    let rule = &refused.deciding[0];
    assert_eq!(rule.id, "mode-task-above-auto");
    assert_eq!(rule.level, "project");
    assert_eq!(
        rule.owner,
        format!("purlis default {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(!rule.file.is_empty(), "its file is named: {rule:?}");
    assert!(rule.line > 0);
    assert!(
        refused
            .rules_said()
            .starts_with("`mode-task-above-auto` (project, purlis default "),
        "{}",
        refused.rules_said()
    );
}

// ── consents ───────────────────────────────────────────────────────────────────────────────

#[test]
fn a_consent_report_or_promote_from_a_chat_is_refused_by_its_rule() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    for action in ["report", "promote"] {
        // Whoever the chat, attended or not, at whichever level: the press is the window's.
        for (sandbox, attended) in [("floor", true), ("full", false), ("off", true)] {
            let refused = decided(
                &strict,
                action,
                ("Plane", "acme"),
                context(sandbox, attended, "default"),
            );
            assert_eq!(refused.outcome, Outcome::Deny, "{action}: {refused:#?}");
            assert_eq!(refused.after_mode, Outcome::Deny, "{action}");
            assert_eq!(refused.family, "consents");
            assert_names(&refused, "consents-window-only");
        }
    }
}

// ── runners ────────────────────────────────────────────────────────────────────────────────

#[test]
fn offloading_to_a_runner_is_judged_by_runners_offload() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    let held = decided(
        &strict,
        "offload",
        ("Runner", "pool-a"),
        context("floor", true, "default"),
    );
    assert_eq!(held.family, "runners");
    assert_eq!(held.outcome, Outcome::Ask, "held for the person: {held:#?}");
    assert_eq!(held.after_mode, Outcome::Ask);
    assert_names(&held, "runners-offload");
    // The person's Approve is once and never "always" (Dispatch #76).
    assert_eq!(held.keep.as_deref(), Some("once"));
    // A forward is another action with a rule of its own.
    let forward = decided(
        &strict,
        "forward",
        ("Runner", "pool-a"),
        context("floor", true, "default"),
    );
    assert_ne!(forward.ids(), held.ids());
}

#[test]
fn forwarding_to_a_runner_is_judged_by_runners_forward() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    let held = decided(
        &strict,
        "forward",
        ("Runner", "pool-a"),
        context("floor", true, "default"),
    );
    assert_eq!(held.family, "runners");
    assert_eq!(held.outcome, Outcome::Ask, "held for the person: {held:#?}");
    assert_names(&held, "runners-forward");
    assert_eq!(held.keep.as_deref(), Some("once"));
    // An unattended chat is held the same way, by the same rule: whether anyone is there to
    // answer is the host's to say, not the rule's (the rule has no `attended` condition).
    let unattended = decide::decide(
        &loaded(STRICT),
        &Request {
            chat: "web/builder",
            of: chat(),
            action: "forward",
            resource: ("Runner", "pool-a"),
            context: context("floor", false, "default"),
        },
        &empty_world(),
    );
    assert_eq!(unattended.outcome, Outcome::Ask, "{unattended:#?}");
    assert_names(&unattended, "runners-forward");
    assert_eq!(unattended.keep.as_deref(), Some("once"));
}

// ── processes ──────────────────────────────────────────────────────────────────────────────

#[test]
fn a_process_deny_beyond_the_floor_names_its_rule() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    for sandbox in ["floor", "full"] {
        for program in ["sudo", "doas", "pkexec"] {
            let refused = decided(
                &strict,
                "exec",
                ("Command", program),
                context(sandbox, true, "default"),
            );
            assert_eq!(refused.outcome, Outcome::Deny, "{program}: {refused:#?}");
            assert_eq!(refused.family, "processes");
            assert_names(&refused, "processes-elevation");
        }
    }
    let ordinary = decided(
        &strict,
        "exec",
        ("Command", "ls"),
        context("floor", true, "default"),
    );
    assert_eq!(ordinary.outcome, Outcome::Allow, "{ordinary:#?}");
    // The escape denies are slice 11's own step, never a rule of this family: no rule names one.
    for program in purlis_core::policy::sandbox_level::PROGRAMS {
        let escape = decided(
            &strict,
            "exec",
            ("Command", program),
            context("floor", true, "default"),
        );
        assert!(
            escape.ids().iter().all(|id| *id != "processes-elevation"),
            "{program}: {escape:#?}"
        );
        assert!(
            escape
                .deciding
                .iter()
                .all(|rule| purlis_core::policy::sandbox_level::named_by(
                    "exec", "Command", &rule.id
                )
                .is_none()),
            "{program}"
        );
    }
}

// ── pool caps ──────────────────────────────────────────────────────────────────────────────

#[test]
fn a_pool_cap_is_a_limit_and_log_counts_it() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    let logging = loaded("[policy]\ntemplate = \"strict\"\nlimits = \"log\"\n");
    assert!(!purlis_core::poollimits::all().is_empty());
    for limit in purlis_core::poollimits::all() {
        let key = limit.id.policy_key();
        let spec = purlis_core::policy::limits::spec(key)
            .unwrap_or_else(|| panic!("{key} is a row of the limits key table"));
        let shipped = i64::try_from(limit.default).expect("a default fits");
        assert_eq!(spec.shipped, Shipped::Number(shipped), "{key}");
        assert_eq!(
            spec.loosens,
            Direction::Raised,
            "{key}: a bigger cap loosens"
        );
        assert_eq!(strict.limit(key), Some(shipped), "{key}");
        assert_eq!(strict.limit_verdict(chat(), key, shipped), Verdict::Within);
        assert_eq!(
            strict.limit_verdict(chat(), key, shipped + 1),
            Verdict::Refuse,
            "{key}: past the cap under enforce"
        );
        assert_eq!(
            logging.limit_verdict(chat(), key, shipped + 1),
            Verdict::Counted,
            "{key}: under limits = log it is counted, not applied"
        );
    }
    // Every `runners.pool-*` row of the table is one of the pool's limits, none left over.
    let named: std::collections::BTreeSet<&str> = purlis_core::poollimits::all()
        .iter()
        .map(|limit| limit.id.policy_key())
        .collect();
    let table: std::collections::BTreeSet<&str> = purlis_core::policy::limits::table()
        .iter()
        .map(|spec| spec.key.as_str())
        .filter(|key| key.starts_with("runners.pool-"))
        .collect();
    assert_eq!(named, table);
}

// ── audit retention ────────────────────────────────────────────────────────────────────────

#[test]
fn audit_retention_follows_the_project_limit_within_the_organisation_lock() {
    purlis_core::unsteered!();
    let said = |limits: &str, org: Option<&str>, accepted: Option<&str>| {
        let dir = plane(&format!("[policy.limits]\n{limits}"));
        let org = org.map(organisation);
        levels::load(&sources(dir.path(), org, accepted)).audit_retention()
    };

    // Shipped: 30 days of event log, a year of record, 2 GiB (spec §5.1).
    let shipped = loaded(STRICT).audit_retention();
    assert_eq!(shipped.event_log_days, 30);
    assert_eq!(shipped.record_days, 365);
    assert_eq!(shipped.record_max_bytes, 2 * 1024 * 1024 * 1024);

    // A project keeping longer tightens, and applies at once.
    let longer = said(
        "\"audit.event-log-days\" = 90\n\"audit.record-days\" = 500\n",
        None,
        None,
    );
    assert_eq!((longer.event_log_days, longer.record_days), (90, 500));

    // Keeping less is a loosening: it waits for the press, so the accepted copy's number stands.
    let shorter = said("\"audit.event-log-days\" = 14\n", None, None);
    assert_eq!(shorter.event_log_days, 30);

    // The Organisation's lock binds the project below it, in both directions.
    let lock = "[policy.limits]\n\"audit.event-log-days\" = 60\n";
    let below = said(
        "\"audit.event-log-days\" = 45\n",
        Some(lock),
        Some("[policy.limits]\n\"audit.event-log-days\" = 45\n"),
    );
    assert_eq!(below.event_log_days, 60, "never below the Organisation's");
    let above = said("\"audit.event-log-days\" = 75\n", Some(lock), None);
    assert_eq!(above.event_log_days, 75, "a project may keep longer");
    // The lock holds the record's own numbers the same way.
    let record = "[policy.limits]\n\"audit.record-days\" = 400\n";
    let kept = said(
        "\"audit.record-days\" = 100\n",
        Some(record),
        Some("[policy.limits]\n\"audit.record-days\" = 100\n"),
    );
    assert_eq!(kept.record_days, 400);
}

#[test]
fn audit_retention_below_its_minimum_reads_as_the_minimum_with_a_notice() {
    purlis_core::unsteered!();
    // Said below the least by the project as committed and by what the person accepted, so no
    // other side holds it up: the number in force reads as the minimum.
    let low = "[policy.limits]\n\"audit.event-log-days\" = 3\n\"audit.record-days\" = 7\n\
               \"audit.record-max-bytes\" = 1048576\n";
    let dir = plane(low);
    let loaded = levels::load(&sources(dir.path(), None, Some(low)));
    let retention = loaded.audit_retention();
    assert_eq!(retention.event_log_days, 7, "at least 7 days");
    assert_eq!(retention.record_days, 30, "at least 30 days");
    assert_eq!(
        retention.record_max_bytes,
        64 * 1024 * 1024,
        "at least 64 MiB"
    );
    for (key, least) in [
        ("audit.event-log-days", 7),
        ("audit.record-days", 30),
        ("audit.record-max-bytes", 64 * 1024 * 1024),
    ] {
        let notice = loaded
            .notices
            .iter()
            .find(|notice| notice.what.contains(key))
            .unwrap_or_else(|| panic!("a notice names {key}: {:#?}", loaded.notices));
        assert_eq!(notice.file, "purlis.toml");
        assert!(
            notice.what.contains(&format!("reads as {least}")),
            "{}",
            notice.what
        );
        assert!(notice.line > 0);
    }
    // Over its most, the event log reads as the most, with a notice.
    let high = "[policy.limits]\n\"audit.event-log-days\" = 400\n";
    let dir = plane(high);
    let loaded = levels::load(&sources(dir.path(), None, Some(high)));
    assert_eq!(loaded.audit_retention().event_log_days, 90, "90 at most");
    assert!(
        loaded
            .notices
            .iter()
            .any(|notice| notice.what.contains("audit.event-log-days")
                && notice.what.contains("reads as 90")),
        "{:#?}",
        loaded.notices
    );
}

#[test]
fn no_policy_can_turn_the_audit_record_off() {
    purlis_core::unsteered!();
    // Zeros said by the project as committed and by what it accepted, in a file that reads clean
    // (so the numbers are really read).
    let off = "[policy.limits]\n\"audit.event-log-days\" = 0\n\"audit.record-days\" = 0\n\
               \"audit.record-max-bytes\" = 0\n";
    let dir = plane(off);
    let loaded = levels::load(&sources(dir.path(), None, Some(off)));
    assert_eq!(
        loaded.limit("audit.record-days"),
        Some(30),
        "read, and held to its least"
    );
    // The family never leaves `enforce`, in every template.
    for template in ["open", "watch", "hold", "team", "strict"] {
        let doc = format!("[policy]\ntemplate = \"{template}\"\naudit = \"off\"\n");
        let loaded = self::loaded(&doc);
        assert_eq!(
            loaded.mode_for("audit", chat()),
            Mode::Enforce,
            "{template}: audit = off reads as enforce"
        );
    }
    // And its numbers never reach zero: there is always an event log and a record to read.
    let retention = loaded.audit_retention();
    assert!(retention.event_log_days >= 7, "{retention:?}");
    assert!(retention.record_days >= 30, "{retention:?}");
    assert!(
        retention.record_max_bytes >= 64 * 1024 * 1024,
        "{retention:?}"
    );
    // `limits = off` (a file of its own: the key is `limits` either way) counts nothing against
    // retention: it is a floor, not a cap that was met.
    let limits_off = self::loaded("[policy]\nlimits = \"off\"\n");
    assert_eq!(limits_off.mode_for("limits", chat()), Mode::Off);
    assert_eq!(limits_off.mode_for("audit", chat()), Mode::Enforce);
    assert_eq!(
        limits_off.audit_retention(),
        self::loaded(STRICT).audit_retention()
    );
}

// ── round 2 of #7516 ───────────────────────────────────────────────────────────────────────

#[test]
fn a_task_whose_start_mode_the_host_cannot_read_is_allowed_as_unknown_for_now() {
    purlis_core::unsteered!();
    let dir = plane(STRICT);
    // Opencode's mode is never read, and a Codex start with no bypass word records none: both
    // read `unknown`, which `mode-task-above-auto` lets through. Known gap, owed to slice 37.
    for harness in [Harness::Codex, Harness::Opencode] {
        let task = Fact::at_start(Begun::Task, Some(harness), &["a-harness".to_owned()]);
        assert_eq!(decide::harness_mode_word(task.start_mode()), "unknown");
        let entered = enter_for(dir.path(), &facts_of(task));
        assert_eq!(entered.outcome, Outcome::Allow, "{harness:?}: {entered:#?}");
    }
    // Where the host does know the words are a bypass, a Codex task is refused as Claude Code's is.
    let codex = Fact::at_start(
        Begun::Task,
        Some(Harness::Codex),
        &[
            "codex".to_owned(),
            "--dangerously-bypass-approvals-and-sandbox".to_owned(),
        ],
    );
    let refused = enter_for(dir.path(), &facts_of(codex));
    assert_eq!(refused.after_mode, Outcome::Deny, "{refused:#?}");
    assert_names(&refused, "mode-task-above-auto");
    // And a Claude Code task started with the skip flag, not `--permission-mode`.
    let claude = a_task_started_on(&["claude", "--dangerously-skip-permissions"]);
    assert_eq!(
        enter_for(dir.path(), &facts_of(claude)).after_mode,
        Outcome::Deny
    );
}

#[test]
fn a_project_rule_on_start_reads_the_real_start_mode_and_attendance() {
    purlis_core::unsteered!();
    use purlis_core::knowngaps::{self, Declared, Facts, Judged, Ruling};
    let plane = {
        let dir = tempfile::tempdir().expect("a folder");
        git(dir.path(), &["init", "-q"]);
        std::fs::write(
            dir.path().join("policy.cedar"),
            "// purlis-policy 1\n\
             @id(\"trust-no-start-in-bypass\") @level(\"project\")\n\
             forbid (principal, action == Action::\"start\", resource)\n  \
             when { context.harness_mode == \"bypassPermissions\" };\n\
             @id(\"trust-no-attended-plan-start\") @level(\"project\")\n\
             forbid (principal, action == Action::\"start\", resource)\n  \
             when { context.attended && context.harness_mode == \"plan\" };\n",
        )
        .expect("a rule");
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "c"]);
        dir
    };
    let floor = Resolved::all(Mode::Enforce);
    let facts = Facts::best(Harness::ClaudeCode, purlis_core::sandbox::Os::Linux);
    let declared = Declared::at(&facts, &floor);
    let judge = |attended, harness_mode| {
        let judged = Judged {
            root: plane.path(),
            profile: "claude",
            of: chat(),
            sandbox: "floor",
            attended,
            harness_mode,
        };
        knowngaps::refusal(&judged, &facts, &declared, &floor)
    };
    let bypass = judge(false, Some(ClaudeCodeMode::BypassPermissions));
    match bypass {
        Some(Ruling::Refused(said)) => {
            assert!(said.contains("trust-no-start-in-bypass"), "{said}")
        }
        other => panic!("a start in bypass is refused: {other:?}"),
    }
    assert_eq!(judge(false, Some(ClaudeCodeMode::Default)), None);
    assert_eq!(judge(false, None), None, "unknown is no bypass");
    // The attendance is the real one, not `true` for every start.
    assert!(judge(true, Some(ClaudeCodeMode::Plan)).is_some());
    assert_eq!(judge(false, Some(ClaudeCodeMode::Plan)), None);
}

#[test]
fn elevation_is_refused_by_its_program_whatever_the_path_it_is_named_by() {
    purlis_core::unsteered!();
    let strict = loaded(STRICT);
    for program in [
        "/usr/bin/sudo",
        "/usr/local/bin/doas",
        "/usr/bin/pkexec",
        "su",
        "/bin/su",
        "sudoedit",
        "/usr/bin/sudoedit",
        "run0",
        "/usr/bin/run0",
    ] {
        let refused = decided(
            &strict,
            "exec",
            ("Command", program),
            context("floor", true, "default"),
        );
        assert_eq!(refused.outcome, Outcome::Deny, "{program}: {refused:#?}");
        assert_names(&refused, "processes-elevation");
    }
    for program in [
        "/usr/bin/sudo-not",
        "/usr/bin/suffix",
        "/usr/bin/ls",
        "issue",
    ] {
        let ordinary = decided(
            &strict,
            "exec",
            ("Command", program),
            context("floor", true, "default"),
        );
        assert_eq!(ordinary.outcome, Outcome::Allow, "{program}: {ordinary:#?}");
    }
}

#[test]
fn elevation_is_refused_at_the_floor_and_full_and_not_at_off() {
    purlis_core::unsteered!();
    // `processes` is only what the `floor` or `full` sandbox adds (spec §3, the families table),
    // and at `off` there is no wrap, so the family is reported as not in force (spec §6, Q308a).
    // The rule says the same: it holds at `floor` and `full`, and not at `off`.
    assert!(
        purlis_core::policy::sandbox_level::not_in_force(
            purlis_core::enforcement::SandboxLevel::Off
        )
        .contains(&"processes")
    );
    let strict = loaded(STRICT);
    let at_off = decided(
        &strict,
        "exec",
        ("Command", "sudo"),
        context("off", true, "default"),
    );
    assert_ne!(at_off.outcome, Outcome::Deny, "off: {at_off:#?}");
    for sandbox in ["floor", "full"] {
        let held = decided(
            &strict,
            "exec",
            ("Command", "sudo"),
            context(sandbox, true, "default"),
        );
        assert_eq!(held.outcome, Outcome::Deny, "{sandbox}: {held:#?}");
        assert_names(&held, "processes-elevation");
    }
    // The family's own mode is what lifts it.
    let lifted = loaded("[policy]\ntemplate = \"strict\"\nprocesses = \"off\"\n");
    let allowed = decided(
        &lifted,
        "exec",
        ("Command", "sudo"),
        context("floor", true, "default"),
    );
    assert_eq!(allowed.after_mode, Outcome::Allow, "{allowed:#?}");
}

#[test]
fn audit_retention_has_a_most_so_no_consumer_overflows() {
    purlis_core::unsteered!();
    for key in ["audit.record-days", "audit.record-max-bytes"] {
        let spec = purlis_core::policy::limits::spec(key).expect("a key of the table");
        assert!(spec.most.is_some(), "{key} has a most");
    }
    let huge = format!(
        "[policy.limits]\n\"audit.record-days\" = {}\n\"audit.record-max-bytes\" = {}\n",
        i64::MAX,
        i64::MAX
    );
    let dir = plane(&huge);
    let loaded = levels::load(&sources(dir.path(), None, Some(&huge)));
    let retention = loaded.audit_retention();
    assert!(retention.record_days <= 36_500, "{retention:?}");
    assert!(
        retention.record_days.checked_mul(86_400).is_some(),
        "{retention:?}"
    );
    assert!(retention.record_max_bytes <= 1 << 50, "{retention:?}");
    for key in ["audit.record-days", "audit.record-max-bytes"] {
        assert!(
            loaded
                .notices
                .iter()
                .any(|notice| notice.what.contains(key) && notice.what.contains("most")),
            "{key}: {:#?}",
            loaded.notices
        );
    }
}
