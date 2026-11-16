// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::local::OwnConfigHome;
use crate::sandbox::policy::Locks;

const CHAT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

fn said(text: &str) -> Settings {
    Settings::from_text(Some(text))
}

/// A plane whose committed file says `committed`, and whose local file says `local`.
fn plane(committed: &str, local: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("charter.toml"), committed).expect("the committed file");
    if let Some(local) = local {
        std::fs::write(dir.path().join("charter.local.toml"), local).expect("the local file");
    }
    dir
}

#[test]
fn a_project_that_says_nothing_runs_the_watch_template() {
    for settings in [
        Settings::from_text(None),
        said(""),
        said("[plane]\nmode = \"commit\"\n"),
    ] {
        assert_eq!(settings.template, Template::Watch);
        for family in Family::ALL {
            let want = match family {
                Family::Secrets | Family::Vaults | Family::GuardsCredential => Mode::Enforce,
                // The floor is a wrap.
                Family::Sandbox => Mode::Enforce,
                _ => Mode::Log,
            };
            assert_eq!(settings.mode(family), want, "{}", family.key());
        }
    }
    assert_eq!(Template::Watch.sandbox_level(), SandboxLevel::Floor);
}

#[test]
fn each_template_runs_each_family_as_the_ruling_says() {
    let secret = [Family::Secrets, Family::Vaults, Family::GuardsCredential];
    for family in Family::ALL.into_iter().filter(|f| *f != Family::Sandbox) {
        assert_eq!(Template::Open.mode(family), Mode::Off, "{}", family.key());
        assert_eq!(
            Template::Strict.mode(family),
            Mode::Enforce,
            "{}",
            family.key()
        );
        let hold = if secret.contains(&family) || family == Family::GuardsMerge {
            Mode::Enforce
        } else {
            Mode::Log
        };
        assert_eq!(Template::Hold.mode(family), hold, "{}", family.key());
        let team = if secret.contains(&family)
            || matches!(
                family,
                Family::GuardsMerge | Family::GuardsRelease | Family::GuardsSave
            ) {
            Mode::Enforce
        } else {
            Mode::Log
        };
        assert_eq!(Template::Team.mode(family), team, "{}", family.key());
    }
    for template in Template::ALL {
        assert_eq!(
            said(&format!("[policy]\ntemplate = \"{}\"\n", template.word())).template,
            template
        );
    }
    assert_eq!(Template::Open.sandbox_level(), SandboxLevel::Off);
    assert_eq!(Template::Hold.sandbox_level(), SandboxLevel::Floor);
    assert_eq!(Template::Team.sandbox_level(), SandboxLevel::Full);
}

#[test]
fn a_family_overrides_its_template_and_the_guards_are_dotted_keys() {
    let settings =
        said("[policy]\ntemplate = \"open\"\nnetwork = \"enforce\"\nguards.merge = \"log\"\n");
    assert_eq!(settings.mode(Family::Network), Mode::Enforce);
    assert_eq!(settings.mode(Family::GuardsMerge), Mode::Log);
    assert_eq!(settings.mode(Family::GuardsGit), Mode::Off);
    assert!(settings.refused.is_empty(), "{:?}", settings.refused);
    let table = said("[policy.guards]\nrelease = \"off\"\n");
    assert_eq!(table.mode(Family::GuardsRelease), Mode::Off);
}

#[test]
fn a_bad_value_means_enforce_and_is_said() {
    let settings = said("[policy]\ntemplate = \"lenient\"\nnetwork = \"maybe\"\nwifi = \"off\"\n");
    assert_eq!(settings.template, Template::Strict);
    assert_eq!(settings.mode(Family::Network), Mode::Enforce);
    assert_eq!(settings.mode(Family::GuardsGit), Mode::Enforce);
    assert_eq!(settings.refused.len(), 3, "{:?}", settings.refused);
    let not_a_table = said("policy = \"off\"\n");
    assert_eq!(not_a_table.template, Template::Strict);
    assert_eq!(not_a_table.refused.len(), 1);
}

#[test]
fn vaults_are_never_logged() {
    let settings = said("[policy]\ntemplate = \"watch\"\nvaults = \"log\"\n");
    assert_eq!(settings.mode(Family::Vaults), Mode::Enforce);
    assert_eq!(settings.refused.len(), 1, "{:?}", settings.refused);
    for template in Template::ALL {
        assert_ne!(
            template.mode(Family::Vaults),
            Mode::Log,
            "{}",
            template.word()
        );
    }
    assert_eq!(
        said("[policy]\nvaults = \"off\"\n").mode(Family::Vaults),
        Mode::Off
    );
}

#[test]
fn the_sandbox_is_no_key_of_policy() {
    let settings = said("[policy]\ntemplate = \"strict\"\nsandbox = \"off\"\n");
    assert_eq!(settings.mode(Family::Sandbox), Mode::Enforce);
    assert_eq!(settings.refused.len(), 1);
}

#[test]
fn a_family_not_yet_switchable_stays_enforced() {
    let open = said("[policy]\ntemplate = \"open\"\n");
    for family in Family::ALL {
        let want = if family.switchable() {
            Mode::Off
        } else {
            Mode::Enforce
        };
        assert_eq!(
            under(&open, &Locks::none(), family),
            want,
            "{}",
            family.key()
        );
    }
}

#[test]
fn an_admin_lock_wins_over_the_template() {
    let open = said("[policy]\ntemplate = \"open\"\n");
    let locks = Locks::locking_enforcement(Family::GuardsMerge);
    assert_eq!(under(&open, &locks, Family::GuardsMerge), Mode::Enforce);
    assert_eq!(under(&open, &locks, Family::GuardsGit), Mode::Off);
}

#[test]
fn an_existing_lock_keeps_its_own_family_enforced() {
    let open = said("[policy]\ntemplate = \"open\"\n");
    let text = r#"{"sandbox": {"hosts": ["api.example.com"], "socket-grants": false}}"#;
    let locks = Locks::parse(text, Path::new("/etc/purlis/policy.json"));
    assert_eq!(under(&open, &locks, Family::Network), Mode::Enforce);
    assert_eq!(under(&open, &locks, Family::Ports), Mode::Enforce);
    assert_eq!(under(&open, &locks, Family::GuardsGit), Mode::Off);
    // A policy file purlis refuses enforces every family.
    let refused = Locks::parse("not json", Path::new("/etc/purlis/policy.json"));
    for family in Family::ALL {
        assert_eq!(
            under(&open, &refused, family),
            Mode::Enforce,
            "{}",
            family.key()
        );
    }
}

#[test]
fn a_policy_reads_its_enforcement_list_and_refuses_a_word_it_does_not_know() {
    let path = Path::new("/etc/purlis/policy.json");
    let locks = Locks::parse(r#"{"enforcement": ["guards.release"]}"#, path);
    assert_eq!(locks.refused_because(), None);
    assert_eq!(
        locks.enforcement_floor(Family::GuardsRelease),
        Some(Mode::Enforce)
    );
    assert_eq!(locks.enforcement_floor(Family::Skills), None);
    assert!(
        Locks::parse(r#"{"enforcement": ["wifi"]}"#, path)
            .refused_because()
            .is_some()
    );
    assert!(
        Locks::parse(r#"{"enforcement": {"network": "log"}}"#, path)
            .refused_because()
            .is_some()
    );
}

#[test]
fn the_local_file_can_tighten_but_never_loosen() {
    let tighter = plane(
        "[policy]\ntemplate = \"watch\"\n",
        Some("[policy]\nguards.git = \"enforce\"\n"),
    );
    assert_eq!(
        Settings::read(tighter.path()).mode(Family::GuardsGit),
        Mode::Enforce
    );
    let looser = plane(
        "[policy]\ntemplate = \"strict\"\n",
        Some("[policy]\ntemplate = \"open\"\n"),
    );
    for family in Family::ALL {
        assert_eq!(
            Settings::read(looser.path()).mode(family),
            Mode::Enforce,
            "{}",
            family.key()
        );
    }
}

#[test]
fn a_loosening_waits_for_the_persons_press_and_a_tightening_does_not() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n", None);
    // The first start takes strict, which is tighter than the default, at once.
    assert_eq!(at_start(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    // The committed file loosens to open: the next start is still strict, and the press waits.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("the committed file");
    assert_eq!(at_start(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    assert!(
        waiting(root.path())
            .iter()
            .any(|(family, was, now)| *family == Family::GuardsGit
                && *was == Mode::Enforce
                && *now == Mode::Off)
    );
    // The person presses: the next start runs open.
    press(root.path()).expect("the press is kept");
    assert_eq!(at_start(root.path()).mode(Family::GuardsGit), Mode::Off);
    assert!(waiting(root.path()).is_empty());
}

#[test]
fn before_any_press_a_project_runs_no_looser_than_watch() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    let resolved = at_start(root.path());
    assert_eq!(resolved.mode(Family::GuardsGit), Mode::Log);
    assert_eq!(resolved.mode(Family::Secrets), Mode::Enforce);
}

#[test]
fn a_hook_takes_the_stricter_of_its_start_and_the_file_now() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"watch\"\n", None);
    let started = at_start(root.path()).spelled();
    let env = |name: &str| (name == ENV).then(|| started.clone());
    assert_eq!(
        for_this_chat(root.path(), &env).mode(Family::GuardsGit),
        Mode::Log
    );
    // Tightened mid-chat: at once.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .expect("the committed file");
    assert_eq!(
        for_this_chat(root.path(), &env).mode(Family::GuardsGit),
        Mode::Enforce
    );
}

#[test]
fn a_branch_folders_file_never_loosens_what_the_chat_started_with() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n", None);
    let started = at_start(root.path()).spelled();
    // A branch folder of the plane carries its own committed file, saying open.
    let branch = plane("[policy]\ntemplate = \"open\"\n", None);
    let env = |name: &str| (name == ENV).then(|| started.clone());
    for family in Family::ALL {
        assert_eq!(
            for_this_chat(branch.path(), &env).mode(family),
            Mode::Enforce,
            "{}",
            family.key()
        );
    }
}

#[test]
fn a_chats_own_environment_never_loosens_what_the_person_accepted() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n", None);
    at_start(root.path());
    // The project loosens to open, unpressed; the chat's own command names everything off.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("the committed file");
    let all_off = Resolved::all(Mode::Off).spelled();
    let env = |name: &str| (name == ENV).then(|| all_off.clone());
    assert_eq!(
        for_this_chat(root.path(), &env).mode(Family::GuardsGit),
        Mode::Enforce
    );
}

#[test]
fn the_resolved_policy_reads_back_as_it_was_spelled_and_a_missing_family_is_enforced() {
    let resolved = Resolved::of(&said("[policy]\ntemplate = \"team\"\n"), &Locks::none());
    assert_eq!(Resolved::parse(&resolved.spelled()), resolved);
    assert_eq!(
        Resolved::parse("network=off").mode(Family::GuardsGit),
        Mode::Enforce
    );
    assert_eq!(
        Resolved::parse("network=off").mode(Family::Network),
        Mode::Off
    );
}

#[test]
fn a_logged_refusal_is_one_quiet_line_of_rule_family_chat_and_host() {
    let plane = tempfile::tempdir().expect("a folder");
    let allowed = judged(
        plane.path(),
        Some(CHAT),
        Family::Network,
        Mode::Log,
        "egress-refused",
        Some("example.com"),
        "refused",
    );
    assert_eq!(allowed, None);
    let log = read_log(plane.path(), CHAT);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].rule, "egress-refused");
    assert_eq!(log[0].family, "network");
    assert_eq!(log[0].chat, CHAT);
    assert_eq!(log[0].host.as_deref(), Some("example.com"));
    let line =
        std::fs::read_to_string(log_file(plane.path(), CHAT).expect("a file")).expect("the log");
    let keys: Vec<String> =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(line.trim())
            .expect("one object")
            .keys()
            .cloned()
            .collect();
    assert_eq!(keys, ["at", "rule", "family", "chat", "host"]);
}

#[test]
fn an_off_refusal_allows_and_writes_nothing_and_an_enforced_one_refuses() {
    let plane = tempfile::tempdir().expect("a folder");
    let off = judged(
        plane.path(),
        Some(CHAT),
        Family::Skills,
        Mode::Off,
        "r",
        None,
        1,
    );
    assert_eq!(off, None);
    let enforced = judged(
        plane.path(),
        Some(CHAT),
        Family::Skills,
        Mode::Enforce,
        "r",
        None,
        1,
    );
    assert_eq!(enforced, Some(1));
    assert!(!log_dir(plane.path()).exists());
}

#[test]
fn secrets_under_watch_still_mask() {
    let _home = OwnConfigHome::new();
    let values = vec!["s3cr3t-value".to_owned()];
    for text in [
        "",
        "[policy]\ntemplate = \"watch\"\n",
        "[policy]\ntemplate = \"hold\"\n",
    ] {
        let root = plane(text, None);
        at_start(root.path());
        let masked = crate::secrets::masked_of(root.path(), Some(CHAT), &values);
        assert_eq!(masked, values.as_slice(), "{text:?}");
        assert_eq!(
            crate::secrets::redact(b"token s3cr3t-value", masked),
            b"token ***"
        );
    }
}

#[test]
fn secrets_off_or_logged_stop_masking_only_once_the_person_pressed() {
    let _home = OwnConfigHome::new();
    let values = vec!["s3cr3t-value".to_owned()];
    for (mode, lines) in [(Mode::Off, 0), (Mode::Log, 1)] {
        let root = plane(&format!("[policy]\nsecrets = \"{}\"\n", mode.word()), None);
        // Not pressed: still masked, and nothing logged.
        at_start(root.path());
        assert_eq!(
            crate::secrets::masked_of(root.path(), Some(CHAT), &values),
            values.as_slice()
        );
        press(root.path()).expect("the press is kept");
        assert!(crate::secrets::masked_of(root.path(), Some(CHAT), &values).is_empty());
        assert_eq!(read_log(root.path(), CHAT).len(), lines, "{}", mode.word());
    }
}

#[test]
fn an_acceptance_whose_seal_does_not_hold_counts_as_none() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    at_start(root.path());
    press(root.path()).expect("the press is kept");
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Off);
    // Something other than the app writes an acceptance of its own beside the seal.
    let file = crate::sandbox::local::path(root.path());
    let text = std::fs::read_to_string(&file).expect("this machine's file");
    let mut doc: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    doc["policy_accepted"]["guards.release"] = serde_json::json!("enforce");
    std::fs::write(&file, doc.to_string()).expect("written");
    assert!(crate::sandbox::local::policy_unsealed(root.path()));
    // No acceptance: the shipped default stands until the person presses again.
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Log);
    assert_eq!(at_start(root.path()).mode(Family::GuardsGit), Mode::Log);
}

#[test]
fn a_process_that_cannot_seal_cannot_accept_a_loosening() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_for_this_test(None);
    let pressed = press(root.path());
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert!(pressed.is_err());
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Log);
}
