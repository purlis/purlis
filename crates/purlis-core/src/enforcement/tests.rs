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
    assert_eq!(Template::Watch.sandbox_level(), Some(SandboxLevel::Floor));
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
            || family.added_by_q308b()
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
    assert_eq!(Template::Open.sandbox_level(), Some(SandboxLevel::Off));
    assert_eq!(Template::Hold.sandbox_level(), Some(SandboxLevel::Floor));
    assert_eq!(Template::Team.sandbox_level(), Some(SandboxLevel::Full));
    // Strict is today's purlis: `[sandbox]` decides, the floor unless its mode is on (Q316).
    assert_eq!(Template::Strict.sandbox_level(), None);
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
fn a_bad_value_means_strict_and_is_said() {
    for text in [
        "[policy]\ntemplate = \"open\"\nnetwork = \"maybe\"\n",
        "[policy]\ntemplate = \"open\"\nwifi = \"off\"\n",
        "[policy]\ntemplate = \"lenient\"\n",
        "[policy\nthis is not toml",
    ] {
        let settings = said(text);
        assert_eq!(settings.template, Template::Strict, "{text:?}");
        for family in Family::ALL {
            assert_eq!(
                settings.mode(family),
                Mode::Enforce,
                "{text:?} {}",
                family.key()
            );
        }
        assert!(!settings.refused.is_empty(), "{text:?}");
    }
}

#[test]
fn a_bad_value_is_said_one_sentence_each() {
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
    press(root.path(), &digest_of(root.path())).expect("the press is kept");
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

/// Every family enforced.
fn strict() -> Resolved {
    Resolved::all(Mode::Enforce)
}

#[test]
fn a_process_that_is_not_the_app_and_has_no_answer_runs_strict() {
    // The review of #6992, B2: no seal key in this thread, no answer from the app.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    at_start(root.path());
    press(root.path(), &digest_of(root.path())).expect("pressed in the app");
    crate::secrets::seal::set_for_this_test(None);
    set_in_the_app_for_this_test(false);
    let resolved = in_force(root.path());
    set_in_the_app_for_this_test(true);
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert_eq!(resolved, strict());
}

#[test]
fn a_process_that_is_not_the_app_takes_the_apps_answer_and_never_the_folder_it_names() {
    let _home = OwnConfigHome::new();
    // The folder a process might name as its plane says strict; the app answered watch, and
    // its answer is all there is: the folder is never read.
    let named = plane("[policy]\ntemplate = \"strict\"\n", None);
    let watch = Resolved::of(&Settings::default(), &Locks::none());
    crate::secrets::seal::set_for_this_test(None);
    set_in_the_app_for_this_test(false);
    answered_by_the_app(Some(watch.clone()));
    let resolved = in_force(named.path());
    set_in_the_app_for_this_test(true);
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert_eq!(resolved, watch);
    assert_eq!(resolved.mode(Family::GuardsGit), Mode::Log);
}

#[test]
fn in_the_app_a_press_reaches_every_chat_at_once() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n", None);
    at_start(root.path());
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("the committed file");
    // Loosened in the file, not pressed: still strict, and a tightening is never waited for.
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
}

#[test]
fn a_press_on_a_file_that_changed_since_it_was_shown_accepts_nothing() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"hold\"\n", None);
    at_start(root.path());
    let shown = digest_of(root.path());
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("changed after it was shown");
    assert!(press(root.path(), &shown).is_err());
    assert_eq!(
        accepted(root.path()).mode(Family::GuardsMerge),
        Mode::Enforce
    );
}

#[test]
fn the_committed_file_is_read_at_head_not_as_the_working_tree_has_it() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n", None);
    let git = |args: &[&str]| {
        let mut git = std::process::Command::new("git");
        git.arg("-C")
            .arg(root.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let ran = crate::forklock::output(&mut git).expect("git runs");
        assert!(ran.status.success(), "{args:?}: {ran:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "charter.toml"]);
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@example.invalid",
        "commit",
        "-q",
        "-m",
        "p",
    ]);
    // Loosened in the working tree, never committed: not the project's.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("the working tree");
    assert_eq!(Settings::read(root.path()).template, Template::Strict);
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
        press(root.path(), &digest_of(root.path())).expect("the press is kept");
        assert!(crate::secrets::masked_of(root.path(), Some(CHAT), &values).is_empty());
        assert_eq!(read_log(root.path(), CHAT).len(), lines, "{}", mode.word());
    }
}

#[test]
fn an_acceptance_whose_seal_does_not_hold_counts_as_none() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    at_start(root.path());
    press(root.path(), &digest_of(root.path())).expect("the press is kept");
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Off);
    // Something other than the app writes an acceptance of its own beside the seal.
    let file = crate::sandbox::local::path(root.path());
    let text = std::fs::read_to_string(&file).expect("this machine's file");
    let mut doc: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    doc["policy_accepted"]["guards.release"] = serde_json::json!("enforce");
    std::fs::write(&file, doc.to_string()).expect("written");
    assert!(crate::sandbox::local::policy_unsealed(root.path()));
    // One was kept, and this is not it: strict until the person presses again.
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    assert_eq!(at_start(root.path()).mode(Family::GuardsGit), Mode::Enforce);
}

#[test]
fn a_process_that_cannot_seal_cannot_accept_a_loosening() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_for_this_test(None);
    let pressed = press(root.path(), &digest_of(root.path()));
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert!(pressed.is_err());
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Log);
}

/// The machine file's text, and a way to put it back.
fn machine_file(root: &Path) -> std::path::PathBuf {
    crate::sandbox::local::path(root)
}

#[test]
fn an_acceptance_removed_after_one_was_kept_reads_as_strict() {
    // The review of #6992's M75 round, B1: deleting the sealed record never drops a forbid.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    let file = machine_file(root.path());
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("kept")).expect("JSON");
    let object = doc.as_object_mut().expect("an object");
    object.remove("policy_accepted");
    object.remove("policy_seal");
    std::fs::write(&file, doc.to_string()).expect("written");
    assert_eq!(in_force(root.path()), Resolved::all(Mode::Enforce));
}

#[test]
fn an_older_sealed_acceptance_put_back_reads_as_strict() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed open");
    let file = machine_file(root.path());
    let open_copy = std::fs::read_to_string(&file).expect("kept");
    // The project tightens; the app keeps that as accepted.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .expect("tightened");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    // Something puts back the older, sealed, looser copy, and loosens the file again.
    std::fs::write(&file, open_copy).expect("replayed");
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("loosened");
    assert_eq!(in_force(root.path()), Resolved::all(Mode::Enforce));
}

#[test]
fn a_tightening_the_app_saw_holds_when_the_committed_file_goes_back() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"watch\"\n", None);
    assert_eq!(in_force(root.path()).mode(Family::GuardsMerge), Mode::Log);
    // Tightened, seen once by the app (no chat start in between)...
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"hold\"\n",
    )
    .expect("tightened");
    assert_eq!(
        in_force(root.path()).mode(Family::GuardsMerge),
        Mode::Enforce
    );
    // ...then rolled back: the forbid stays until the person presses.
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"watch\"\n",
    )
    .expect("rolled back");
    assert_eq!(
        in_force(root.path()).mode(Family::GuardsMerge),
        Mode::Enforce
    );
}

#[test]
fn with_no_anchor_a_fresh_install_runs_the_shipped_default() {
    // The M75 review, round 2: no keyring anchor and no record is a fresh install.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    assert_eq!(
        crate::sandbox::local::policy_accepted_record(root.path()),
        crate::sandbox::local::PolicyAccepted::Never
    );
    assert_eq!(
        accepted(root.path()),
        Resolved::of(&Settings::default(), &Locks::none())
    );
}

#[test]
fn a_whole_machine_file_put_back_from_before_a_tightening_reads_as_strict() {
    // The M75 review, round 2: the anchor in the keyring is the newest the app has seen, so a
    // machine file restored whole from before a tightening is older than it.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"watch\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("watch accepted");
    let file = machine_file(root.path());
    let before = std::fs::read_to_string(&file).expect("kept at watch");
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .expect("tightened");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    std::fs::write(&file, before).expect("restored from before");
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"watch\"\n",
    )
    .expect("loosened");
    assert_eq!(in_force(root.path()), Resolved::all(Mode::Enforce));
}

/// A pair nothing covers, as a cross-persona dispatch from `root` would be judged.
fn a_dispatch_across_personas(root: &Path) -> crate::dispatchgrant::Covers {
    crate::dispatchgrant::covers(
        Some("steward"),
        "devops",
        &crate::dispatchgrant::InForce::read(root, Vec::new()),
        &Locks::none(),
    )
}

#[test]
fn a_record_removed_under_its_anchor_refuses_every_dispatch_across_personas() {
    // The M75 review (instruction 63): `strict` alone would drop the forbids and nevers the lost
    // record carried, so a dispatch to another persona is refused until the person presses.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert!(!broken(root.path()));
    let file = machine_file(root.path());
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("kept")).expect("JSON");
    let object = doc.as_object_mut().expect("an object");
    object.remove("policy_accepted");
    object.remove("policy_seal");
    std::fs::write(&file, doc.to_string()).expect("written");
    assert!(broken(root.path()));
    assert_eq!(
        a_dispatch_across_personas(root.path()),
        crate::dispatchgrant::Covers::Unread
    );
    // The press, on the committed file as it stands, mends it.
    press(root.path(), &digest_of(root.path())).expect("pressed again");
    assert!(!broken(root.path()));
    assert_ne!(
        a_dispatch_across_personas(root.path()),
        crate::dispatchgrant::Covers::Unread
    );
}

#[test]
fn a_git_folder_copied_aside_and_back_is_no_fresh_install() {
    // The M75 review, round 3: the clone key carries the identity of `.git`, so copying it aside
    // and back makes a new one. The anchor is keyed by the plane's real path, so the record that
    // clone key lacks is a record gone, not a fresh install.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    let git = |args: &[&str]| {
        let mut git = std::process::Command::new("git");
        git.current_dir(root.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let ran = crate::forklock::output(&mut git).expect("git runs");
        assert!(ran.status.success(), "{args:?}: {ran:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "charter.toml"]);
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@example.invalid",
        "commit",
        "-q",
        "-m",
        "p",
    ]);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    let before = crate::sandbox::local::clone_key(root.path());
    let dot_git = root.path().join(".git");
    let copy = root.path().join("g");
    let mut cp = std::process::Command::new("cp");
    cp.arg("-R").arg(&dot_git).arg(&copy);
    assert!(
        crate::forklock::output(&mut cp)
            .expect("cp runs")
            .status
            .success()
    );
    std::fs::rename(&dot_git, root.path().join("old")).expect("aside");
    std::fs::rename(&copy, &dot_git).expect("back");
    assert_ne!(
        crate::sandbox::local::clone_key(root.path()),
        before,
        "a new clone key"
    );
    assert!(broken(root.path()));
    assert_eq!(in_force(root.path()), Resolved::all(Mode::Enforce));
    assert_eq!(
        a_dispatch_across_personas(root.path()),
        crate::dispatchgrant::Covers::Unread
    );
}

#[test]
fn a_press_after_the_local_file_changed_what_was_shown_accepts_nothing() {
    // The re-review of #6992, S2: the digest is of the modes resolved from HEAD, the local file
    // and the locks, so a change to any of them since the window showed it refuses the press.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    at_start(root.path());
    let shown = digest_of(root.path());
    std::fs::write(
        root.path().join("charter.local.toml"),
        "[policy]\nguards.git = \"enforce\"\n",
    )
    .expect("the local file");
    assert!(press(root.path(), &shown).is_err());
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Log);
    // What the window shows now is what a press accepts.
    let standing = standing(root.path());
    press(root.path(), &standing.digest).expect("pressed on what was shown");
    assert_eq!(accepted(root.path()).mode(Family::GuardsGit), Mode::Enforce);
    assert_eq!(accepted(root.path()).mode(Family::GuardsMerge), Mode::Off);
}

#[test]
fn a_head_git_cannot_read_is_strict_and_never_the_working_tree_file() {
    // The re-review of #6992: a repository whose `HEAD` git cannot read (here, one with no
    // commit) is enforced, whatever its working tree's file says.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    let ran = crate::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["init", "-q"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "{ran:?}");
    let settings = Settings::read(root.path());
    assert_eq!(settings.template, Template::Strict);
    assert!(!settings.refused.is_empty());
    // A plane that is no repository at all reads its file as it stands.
    let plain = plane("[policy]\ntemplate = \"open\"\n", None);
    assert_eq!(Settings::read(plain.path()).template, Template::Open);
}

#[test]
fn a_chats_log_stops_growing_at_its_most_lines() {
    // The re-review of #6992, S3: a chat cannot fill the disk through its log.
    let root = tempfile::tempdir().expect("a folder");
    let path = log_file(root.path(), CHAT).expect("a log file");
    std::fs::create_dir_all(path.parent().expect("its folder")).expect("its folder");
    std::fs::write(&path, "{}\n".repeat(MOST_LINES)).expect("a full log");
    write_line(root.path(), CHAT, Family::GuardsGit, "A3", None);
    let kept = std::fs::read_to_string(&path).expect("the log");
    assert_eq!(kept.lines().count(), MOST_LINES);
}

#[test]
fn a_sealed_acceptance_whose_anchor_is_gone_is_broken() {
    // The re-review of #6992: the app keeps the anchor before the record, so a sealed record
    // the keyring has no anchor for was not kept as it stands.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    crate::secrets::seal::forget_kept_for_this_test();
    assert!(broken(root.path()));
    assert_eq!(in_force(root.path()), strict());
}

#[test]
fn a_seal_key_another_program_made_breaks_every_acceptance_until_the_next_press() {
    // The re-review of #6992, S4: a key the app does not own may be known to whoever made it.
    let _home = OwnConfigHome::new();
    let never = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    let found = (broken(never.path()), in_force(never.path()));
    crate::secrets::seal::set_foreign_for_this_test(false);
    assert!(found.0, "a project never pressed is broken too");
    assert_eq!(found.1, strict());
}
