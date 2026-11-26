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
                // An unapproved profile or declaration still asks (Q329).
                Family::Trust => Mode::Enforce,
                // The person's own credential files are theirs at the floor (Q325).
                Family::CredentialFiles => Mode::Off,
                // The floor is a wrap, and `ports` takes no log (Q328).
                Family::Sandbox | Family::Ports => Mode::Enforce,
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
    for family in Family::ALL
        .into_iter()
        .filter(|f| !matches!(f, Family::Sandbox | Family::CredentialFiles))
    {
        assert_eq!(Template::Open.mode(family), Mode::Off, "{}", family.key());
        assert_eq!(
            Template::Strict.mode(family),
            Mode::Enforce,
            "{}",
            family.key()
        );
        let hold = if secret.contains(&family)
            || family == Family::GuardsMerge
            || family == Family::Ports
            || family == Family::Trust
        {
            Mode::Enforce
        } else {
            Mode::Log
        };
        assert_eq!(Template::Hold.mode(family), hold, "{}", family.key());
        let team = if secret.contains(&family)
            || family == Family::Ports
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

/// Q325: the person's own credential files are held at the floor by `team` and `strict` alone;
/// `open`, `watch` and `hold` leave the person's tools working there.
#[test]
fn credential_files_are_enforced_by_team_and_strict_alone() {
    for (template, mode) in [
        (Template::Open, Mode::Off),
        (Template::Watch, Mode::Off),
        (Template::Hold, Mode::Off),
        (Template::Team, Mode::Enforce),
        (Template::Strict, Mode::Enforce),
    ] {
        assert_eq!(
            Template::mode(template, Family::CredentialFiles),
            mode,
            "{}",
            template.word()
        );
    }
    assert_eq!(Family::CredentialFiles.key(), "secrets.credential_files");
    assert!(Family::CredentialFiles.switchable());
}

/// Q325: a project turns the class on at the floor with `secrets.credential_files = "enforce"`,
/// as a dotted key, a `[policy.secrets]` table, or a quoted key beside `secrets` itself. It takes
/// off or enforce: `log` is enforced, and said.
#[test]
fn a_project_turns_credential_files_on_by_its_key() {
    for text in [
        "[policy]\nsecrets.credential_files = \"enforce\"\n",
        "[policy.secrets]\ncredential_files = \"enforce\"\n",
        "[policy]\nsecrets = \"log\"\n\"secrets.credential_files\" = \"enforce\"\n",
    ] {
        let settings = said(text);
        assert_eq!(
            settings.mode(Family::CredentialFiles),
            Mode::Enforce,
            "{text}"
        );
        assert!(
            settings.refused.is_empty(),
            "{text}: {:?}",
            settings.refused
        );
    }
    // The `secrets` family stays the template's where only the key under it is said.
    let settings = said("[policy]\nsecrets.credential_files = \"enforce\"\n");
    assert_eq!(settings.mode(Family::Secrets), Mode::Enforce);
    assert_eq!(settings.template, Template::Watch);
    let logged = said("[policy]\nsecrets.credential_files = \"log\"\n");
    assert_eq!(logged.mode(Family::CredentialFiles), Mode::Enforce);
    assert_eq!(logged.refused.len(), 1, "{:?}", logged.refused);
    let off = said("[policy]\ntemplate = \"team\"\nsecrets.credential_files = \"off\"\n");
    assert_eq!(off.mode(Family::CredentialFiles), Mode::Off);
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
    // The spelling carries no opt-in (`team` enforces `guards.merge`, so has none), so compare it.
    assert_eq!(
        Resolved::parse(&resolved.spelled()).spelled(),
        resolved.spelled()
    );
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
        let masked = crate::secrets::masked_of(root.path(), (Some(CHAT), None), &values);
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
            crate::secrets::masked_of(root.path(), (Some(CHAT), None), &values),
            values.as_slice()
        );
        press(root.path(), &digest_of(root.path())).expect("the press is kept");
        assert!(crate::secrets::masked_of(root.path(), (Some(CHAT), None), &values).is_empty());
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
    assert_eq!(in_force(root.path()), broken_accepted());
}

#[test]
fn an_acceptance_removed_while_the_seal_key_does_not_read_is_broken_never_none() {
    // #7153: with no key nothing verifies, but the keyring's marker still says one was kept, so
    // a record taken away is broken, never the shipped default, and no accepted deny is dropped.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    std::fs::remove_file(machine_file(root.path())).expect("the record gone");
    crate::secrets::seal::set_for_this_test(None);
    let record = crate::sandbox::local::policy_accepted_record(root.path());
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert_eq!(record, crate::sandbox::local::PolicyAccepted::Broken);
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
    assert_eq!(in_force(root.path()), broken_accepted());
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
    assert_eq!(in_force(root.path()), broken_accepted());
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
    assert_eq!(in_force(root.path()), broken_accepted());
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
    assert_eq!(in_force(root.path()), broken_accepted());
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
    assert_eq!(found.1, broken_accepted());
}

// ---- The Q308 follow-up, item 1: the sandbox level ------------------------------------------

#[test]
fn the_sandbox_level_is_the_sandbox_mode_else_the_templates() {
    for (text, level) in [
        ("", SandboxLevel::Floor),
        ("[policy]\ntemplate = \"open\"\n", SandboxLevel::Off),
        ("[policy]\ntemplate = \"hold\"\n", SandboxLevel::Floor),
        ("[policy]\ntemplate = \"team\"\n", SandboxLevel::Full),
        // Strict leaves it to `[sandbox]`, as before Q308: the floor where it names no mode.
        ("[policy]\ntemplate = \"strict\"\n", SandboxLevel::Floor),
        (
            "[policy]\ntemplate = \"strict\"\n[sandbox]\nmode = \"on\"\n",
            SandboxLevel::Full,
        ),
        (
            "[policy]\ntemplate = \"open\"\n[sandbox]\nmode = \"on\"\n",
            SandboxLevel::Full,
        ),
        (
            "[policy]\ntemplate = \"team\"\n[sandbox]\nmode = \"off\"\n",
            SandboxLevel::Off,
        ),
        // `never` refuses the team's off, and a mode purlis cannot read is `on`.
        (
            "[sandbox]\nmode = \"off\"\npolicy = \"never\"\n",
            SandboxLevel::Full,
        ),
        ("[sandbox]\nmode = \"maybe\"\n", SandboxLevel::Full),
        // A bad `[policy]` value is strict, and `[sandbox]` still decides.
        ("[policy]\ntemplate = \"lenient\"\n", SandboxLevel::Floor),
    ] {
        let settings = said(text);
        assert_eq!(settings.level(), level, "{text:?}");
        assert_eq!(
            settings.mode(Family::Sandbox),
            mode_of_level(level),
            "{text:?}"
        );
        assert_eq!(
            Resolved::of(&settings, &Locks::none()).level(),
            level,
            "{text:?}"
        );
    }
}

#[test]
fn the_local_file_can_raise_the_level_but_never_lower_it() {
    let raised = plane(
        "[policy]\ntemplate = \"watch\"\n",
        Some("[policy]\ntemplate = \"team\"\n"),
    );
    assert_eq!(Settings::read(raised.path()).level(), SandboxLevel::Full);
    let lowered = plane(
        "[policy]\ntemplate = \"team\"\n",
        Some("[policy]\ntemplate = \"open\"\n[sandbox]\nmode = \"off\"\n"),
    );
    assert_eq!(Settings::read(lowered.path()).level(), SandboxLevel::Full);
}

#[test]
fn a_lock_on_the_sandbox_keeps_a_wrap_whatever_the_template() {
    let open = said("[policy]\ntemplate = \"open\"\n");
    let locks = Locks::locking_enforcement(Family::Sandbox);
    let resolved = Resolved::of(&open, &locks);
    assert_eq!(resolved.level(), SandboxLevel::Floor);
    assert_eq!(resolved.mode(Family::Sandbox), Mode::Enforce);
}

#[test]
fn the_level_reads_back_as_spelled_and_an_acceptance_from_before_it_is_the_floor() {
    let team = Resolved::of(&said("[policy]\ntemplate = \"team\"\n"), &Locks::none());
    assert!(
        team.spelled().contains("sandbox=full"),
        "{}",
        team.spelled()
    );
    assert_eq!(Resolved::parse(&team.spelled()).spelled(), team.spelled());
    // #6992 kept the sandbox as a mode: `enforce` was a wrap, at least the floor.
    assert_eq!(
        Resolved::parse("sandbox=enforce").level(),
        SandboxLevel::Floor
    );
    assert_eq!(Resolved::parse("sandbox=off").level(), SandboxLevel::Off);
    // A level it cannot read is the strictest.
    assert_eq!(Resolved::parse("network=off").level(), SandboxLevel::Full);
}

#[test]
fn a_process_that_is_not_the_app_never_runs_a_level_looser_than_the_file_says() {
    let _home = OwnConfigHome::new();
    let named = plane("[policy]\ntemplate = \"open\"\n", None);
    let open = Resolved::of(&said("[policy]\ntemplate = \"open\"\n"), &Locks::none());
    let team = Resolved::of(&said("[policy]\ntemplate = \"team\"\n"), &Locks::none());
    crate::secrets::seal::set_for_this_test(None);
    set_in_the_app_for_this_test(false);
    answered_by_the_app(Some(open));
    let under_open = sandbox_level(named.path(), || SandboxLevel::Full);
    answered_by_the_app(Some(team));
    let under_team = sandbox_level(named.path(), || SandboxLevel::Floor);
    answered_by_the_app(None);
    let unanswered = sandbox_level(named.path(), || SandboxLevel::Off);
    set_in_the_app_for_this_test(true);
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert_eq!(under_open, SandboxLevel::Full);
    assert_eq!(under_team, SandboxLevel::Full);
    // No answer is strict: never below the floor, whatever the file says.
    assert_eq!(unanswered, SandboxLevel::Floor);
}

#[test]
fn a_loosened_level_waits_for_the_press_and_the_window_is_told_it_by_its_words() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"team\"\n", None);
    assert_eq!(at_start(root.path()).level(), SandboxLevel::Full);
    std::fs::write(
        root.path().join("charter.toml"),
        "[policy]\ntemplate = \"watch\"\n",
    )
    .expect("loosened");
    assert_eq!(at_start(root.path()).level(), SandboxLevel::Full);
    let standing = standing(root.path());
    assert_eq!(
        standing.level,
        Some((SandboxLevel::Full, SandboxLevel::Floor))
    );
    assert!(
        standing
            .waiting
            .iter()
            .all(|(family, _, _)| *family != Family::Sandbox)
    );
    press(root.path(), &standing.digest).expect("pressed");
    assert_eq!(at_start(root.path()).level(), SandboxLevel::Floor);
    assert_eq!(level_waiting(root.path()), None);
}

#[test]
fn ports_are_never_logged_the_kernel_tells_nothing_it_let_through() {
    // Ruling Q328: `ports` is off or enforce; `log` is enforced, and said.
    let settings = said("[policy]\ntemplate = \"open\"\nports = \"log\"\n");
    assert_eq!(settings.mode(Family::Ports), Mode::Enforce);
    assert!(!settings.refused.is_empty());
    // Only `ports` is enforced: the rest of the project's policy stands (the review of #7105).
    assert_eq!(settings.template, Template::Open);
    assert_eq!(settings.mode(Family::Network), Mode::Off);
    assert_eq!(settings.mode(Family::GuardsGit), Mode::Off);
    // A value purlis cannot read still makes the whole policy strict.
    let unread = said("[policy]\ntemplate = \"open\"\nports = \"loud\"\n");
    assert_eq!(unread.template, Template::Strict);
    for template in Template::ALL {
        assert_ne!(
            template.mode(Family::Ports),
            Mode::Log,
            "{}",
            template.word()
        );
    }
    assert_eq!(
        said("[policy]\nports = \"off\"\n").mode(Family::Ports),
        Mode::Off
    );
}

#[test]
fn a_team_off_answer_the_project_never_committed_never_lowers_the_accepted_level() {
    // The review of #7105: the team's-off answer is kept unsealed, so it lowers the sealed level
    // only for an off the project committed and still says, which this person accepted.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"team\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(accepted(root.path()).level(), SandboxLevel::Full);
    crate::sandbox::local::answer_team_off(
        root.path(),
        crate::sandbox::local::TeamOffAnswer { accepted: true },
    )
    .expect("an answer written beside the seal");
    assert_eq!(accepted(root.path()).level(), SandboxLevel::Full);
    assert_eq!(at_start(root.path()).level(), SandboxLevel::Full);
}

#[test]
fn a_broken_acceptance_runs_the_full_sandbox_never_the_floor() {
    // The review of #7105: a broken acceptance is never looser than the full sandbox it may have
    // kept.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"team\"\n", None);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    crate::secrets::seal::forget_kept_for_this_test();
    assert!(broken(root.path()));
    assert_eq!(accepted(root.path()).level(), SandboxLevel::Full);
    assert_eq!(accepted(root.path()), broken_accepted());
}

/// **The `vaults` family** (item 4 of the Q308 follow-up): off or enforce only. `open`, or
/// `vaults = "off"` once the person pressed, opens a registered vault to a chat of any persona;
/// `strict`, `watch`, `hold` and `team` keep a vault to the personas it is tagged for.
#[test]
fn vaults_off_opens_a_registered_vault_and_watch_hold_and_team_keep_it_enforced() {
    use crate::secrets::{Ctx, Env, brokered};
    let _home = OwnConfigHome::new();
    for (text, opened) in [
        ("[policy]\ntemplate = \"strict\"\n", false),
        ("[policy]\ntemplate = \"watch\"\n", false),
        ("[policy]\ntemplate = \"hold\"\n", false),
        ("[policy]\ntemplate = \"team\"\n", false),
        ("[policy]\ntemplate = \"strict\"\nvaults = \"off\"\n", true),
        ("[policy]\ntemplate = \"open\"\n", true),
    ] {
        let root = plane(text, None);
        std::fs::write(
            root.path().join("vaults.json"),
            serde_json::json!({ "vaults": {
                "prod": {"provider": "plain-file", "config": {"file": "prod.json"}, "persona": "devops"},
            }})
            .to_string(),
        )
        .expect("the registry");
        at_start(root.path());
        press(root.path(), &digest_of(root.path())).expect("the press is kept");
        assert_eq!(
            mode(root.path(), Family::Vaults) == Mode::Off,
            opened,
            "{text}"
        );
        let ctx = Ctx::new(root.path(), Env::of(&[]));
        assert_eq!(
            brokered::authorise(&ctx, Some("steward"), "prod").is_ok(),
            opened,
            "{text}"
        );
        // Its own persona, always; a vault the project does not register, never.
        assert_eq!(brokered::authorise(&ctx, Some("devops"), "prod"), Ok(()));
        assert!(
            brokered::authorise(&ctx, Some("devops"), "nope").is_err(),
            "{text}"
        );
    }
}

/// A plane whose project lets masking and the vaults check go, pressed: and the chain that
/// dispatched a chat enforced both (the review of #7113).
fn loosened_plane() -> tempfile::TempDir {
    let root = plane("[policy]\nsecrets = \"off\"\nvaults = \"off\"\n", None);
    at_start(root.path());
    press(root.path(), &digest_of(root.path())).expect("the press is kept");
    root
}

const STRICT_CHAIN: &str = "secrets=enforce,vaults=enforce";

#[test]
fn a_dispatched_chat_s_output_is_masked_where_its_chain_masked_it() {
    let _home = OwnConfigHome::new();
    let values = vec!["s3cr3t-value".to_owned()];
    let root = loosened_plane();
    assert!(crate::secrets::masked_of(root.path(), (Some(CHAT), None), &values).is_empty());
    assert_eq!(
        crate::secrets::masked_of(root.path(), (Some(CHAT), Some(STRICT_CHAIN)), &values),
        values.as_slice()
    );
}

#[test]
fn a_dispatched_chat_s_vaults_are_checked_where_its_chain_checked_them() {
    let _home = OwnConfigHome::new();
    let root = loosened_plane();
    let ctx = crate::secrets::Ctx::new(root.path(), crate::secrets::Env::of(&[]));
    assert!(crate::secrets::brokered::vaults_off(&ctx));
    assert!(!crate::secrets::brokered::vaults_off(
        &ctx.under_chain(Some(STRICT_CHAIN))
    ));
}

#[test]
fn every_family_q308b_added_switches_under_open_and_keeps_its_template_defaults() {
    // Q308b, item 7 of the follow-up: the nine families switch, each at its template's mode.
    let nine = Family::ALL
        .into_iter()
        .filter(|family| family.added_by_q308b())
        .collect::<Vec<_>>();
    assert_eq!(nine.len(), 9);
    for family in nine {
        assert!(family.switchable(), "{}", family.key());
        // Q329: `trust` is enforced under every template but `open`.
        let floor = if family == Family::Trust {
            Mode::Enforce
        } else {
            Mode::Log
        };
        for (template, want) in [
            ("open", Mode::Off),
            ("watch", floor),
            ("hold", floor),
            ("team", Mode::Enforce),
            ("strict", Mode::Enforce),
        ] {
            let settings = said(&format!("[policy]\ntemplate = \"{template}\"\n"));
            assert_eq!(
                under(&settings, &Locks::none(), family),
                want,
                "{template} {}",
                family.key()
            );
        }
        let off = said(&format!(
            "[policy]\ntemplate = \"strict\"\n{} = \"off\"\n",
            family.key()
        ));
        assert_eq!(
            under(&off, &Locks::none(), family),
            Mode::Off,
            "{}",
            family.key()
        );
        // An administrator's lock still wins.
        assert_eq!(
            under(&off, &Locks::locking_enforcement(family), family),
            Mode::Enforce,
            "{}",
            family.key()
        );
    }
}

#[test]
fn the_rules_of_the_nine_families_are_known_to_their_own_family_alone() {
    // The app keeps a line a chat's process hands over only for a rule it knows (#7061).
    for (family, rule) in [
        (Family::GuardsForge, RULE_FORGE_WORKSPACE),
        (Family::HarnessMode, RULE_ASKS_NOBODY),
        (Family::Tools, RULE_DENY_LIST),
        (Family::Trust, RULE_PROFILE_APPROVAL),
        (Family::Trust, RULE_DECLARATION_APPROVAL),
        (Family::Runners, RULE_RUNNER_DISPATCH),
        (Family::Runners, RULE_RUNNER_WRITE),
        (Family::Consents, RULE_WINDOW_ONLY_ASK),
        (Family::Limits, RULE_COMMIT_RATE),
        (Family::Limits, "dispatch.depth"),
        (Family::Limits, "dispatch.running-per-chat"),
        (Family::GuardsPlane, "A3"),
        (Family::GuardsShape, "caps"),
    ] {
        assert!(known_rule(family, rule), "{} {rule}", family.key());
        assert!(!known_rule(Family::Dispatch, rule), "{rule}");
    }
    assert!(!known_rule(Family::Limits, "dispatch.anything"));
    assert!(!known_rule(Family::GuardsGit, "A3"));
}

#[test]
fn a_project_that_named_guards_git_keeps_the_plane_roots_arms_as_strict_after_their_move() {
    // #7104: A3 and A3b moved from `guards.git` to `guards.plane` (Q308b). A project on `watch`
    // that enforced `guards.git` keeps them enforced; one that names `guards.plane` decides.
    for (text, want) in [
        ("[policy]\nguards.git = \"enforce\"\n", Mode::Enforce),
        (
            "[policy]\ntemplate = \"hold\"\nguards.git = \"enforce\"\n",
            Mode::Enforce,
        ),
        ("[policy]\n", Mode::Log),
        (
            "[policy]\ntemplate = \"strict\"\nguards.git = \"off\"\n",
            Mode::Enforce,
        ),
        (
            "[policy]\nguards.git = \"enforce\"\nguards.plane = \"log\"\n",
            Mode::Log,
        ),
    ] {
        assert_eq!(
            under(&said(text), &Locks::none(), Family::GuardsPlane),
            want,
            "{text}"
        );
    }
    // An administrator's lock on `guards.git` held A3 and A3b at enforce before the move, and
    // still does, whatever the project says of either family (the re-review of #7104).
    assert_eq!(
        under(
            &said("[policy]\nguards.shape = \"off\"\n"),
            &Locks::parse(
                r#"{"enforcement": ["guards.git"]}"#,
                Path::new("/etc/purlis/policy.json")
            ),
            Family::GuardsShape
        ),
        Mode::Off,
        "a lock on guards.git leaves guards.shape the project's"
    );
    let git_locked = Locks::parse(
        r#"{"enforcement": ["guards.git"]}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    for text in [
        "[policy]\n",
        "[policy]\ntemplate = \"open\"\n",
        "[policy]\nguards.git = \"off\"\nguards.plane = \"off\"\n",
        "[policy]\nguards.shape = \"off\"\n",
    ] {
        assert_eq!(
            under(&said(text), &git_locked, Family::GuardsPlane),
            Mode::Enforce,
            "{text}"
        );
        // Only `guards.plane` moves with it: `guards.shape` stays the project's to switch, so a
        // refused lock file (which locks everything) cannot pass this test (#7155).
        assert_eq!(
            under(&said(text), &git_locked, Family::GuardsShape),
            under(&said(text), &Locks::none(), Family::GuardsShape),
            "{text}"
        );
    }
}

/// `git <args>` in `root`, with no configuration but the repository's.
fn git_in(root: &Path, args: &[&str]) -> std::process::Output {
    let mut git = std::process::Command::new("git");
    git.arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    crate::forklock::output(&mut git).expect("git runs")
}

/// A plane whose file says `committed`, committed at `HEAD`.
fn a_committed_plane(committed: &str) -> tempfile::TempDir {
    let root = plane(committed, None);
    for args in [
        &["init", "-q"][..],
        &["add", "charter.toml"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "p",
        ],
    ] {
        let ran = git_in(root.path(), args);
        assert!(ran.status.success(), "{args:?}: {ran:?}");
    }
    root
}

/// The object file of the committed settings file's blob, which git cannot read once it is gone.
fn the_files_blob(root: &Path) -> PathBuf {
    let ran = git_in(root, &["rev-parse", "HEAD:charter.toml"]);
    assert!(ran.status.success(), "{ran:?}");
    let sha = String::from_utf8(ran.stdout).expect("utf-8");
    let sha = sha.trim();
    root.join(".git/objects").join(&sha[..2]).join(&sha[2..])
}

#[test]
fn a_blob_git_cannot_read_at_head_is_strict_and_only_a_missing_path_is_no_file() {
    // #7073: `git show` exits 128 for a blob it cannot read as well as for a path that is not
    // there. Only git's own "no such path" reads as a project that says nothing.
    let _home = OwnConfigHome::new();
    let root = a_committed_plane("[policy]\ntemplate = \"open\"\n");
    assert_eq!(Settings::read(root.path()).template, Template::Open);
    std::fs::remove_file(the_files_blob(root.path())).expect("the blob gone");
    let settings = Settings::read(root.path());
    assert_eq!(settings.template, Template::Strict);
    assert!(settings.unread);

    // A file on disk and never committed, and one in no commit at all: no file at `HEAD`.
    let uncommitted = a_committed_plane("");
    let ran = git_in(
        uncommitted.path(),
        &["rm", "-q", "--cached", "charter.toml"],
    );
    assert!(ran.status.success(), "{ran:?}");
    let ran = git_in(
        uncommitted.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "untracked",
        ],
    );
    assert!(ran.status.success(), "{ran:?}");
    std::fs::write(
        uncommitted.path().join("charter.toml"),
        "[policy]\ntemplate = \"open\"\n",
    )
    .expect("on disk");
    let settings = Settings::read(uncommitted.path());
    assert_eq!(settings.template, Template::Watch, "{settings:?}");
    assert!(!settings.unread);
    std::fs::remove_file(uncommitted.path().join("charter.toml")).expect("gone from disk");
    let settings = Settings::read(uncommitted.path());
    assert_eq!(settings.template, Template::Watch, "{settings:?}");
    assert!(!settings.unread);
}

#[test]
fn a_head_git_could_not_read_once_is_never_kept_as_accepted() {
    // #7073: strict while git cannot read `HEAD`, and back to what was pressed once it can,
    // with no press: a git that failed once is no tightening the project made.
    let _home = OwnConfigHome::new();
    let root = a_committed_plane("[policy]\ntemplate = \"open\"\n");
    press(root.path(), &digest_of(root.path())).expect("pressed");
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    // `HEAD` names a branch with no commit for a moment, as a git cut short might leave it.
    let branch = git_in(root.path(), &["symbolic-ref", "HEAD"]);
    let branch = String::from_utf8(branch.stdout).expect("utf-8");
    let ran = git_in(root.path(), &["symbolic-ref", "HEAD", "refs/heads/not-yet"]);
    assert!(ran.status.success(), "{ran:?}");
    assert_eq!(in_force(root.path()), Resolved::strict());
    assert!(in_force(root.path()).denies().is_all());
    let ran = git_in(root.path(), &["symbolic-ref", "HEAD", branch.trim()]);
    assert!(ran.status.success(), "{ran:?}");
    // Every dispatch across personas was denied while it could not read, and that deny is no
    // more kept than the modes are (#7153): nothing is denied, and no deny waits to be lifted.
    assert!(in_force(root.path()).denies().is_empty());
    assert!(standing(root.path()).lifted.is_empty());
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    assert!(waiting(root.path()).is_empty());
}

#[test]
fn a_git_folder_git_cannot_read_is_strict_and_never_the_working_tree_file() {
    // #7073: git calls a plane whose `.git/HEAD` is corrupted no repository, as it calls one
    // with no `.git`. Only one with no `.git` at all reads its working tree's file.
    let _home = OwnConfigHome::new();
    let root = a_committed_plane("[policy]\ntemplate = \"open\"\n");
    std::fs::write(root.path().join(".git/HEAD"), "not a ref\n").expect("a corrupted HEAD");
    assert_eq!(Settings::read(root.path()).template, Template::Strict);
    std::fs::remove_dir_all(root.path().join(".git")).expect("no repository");
    assert_eq!(Settings::read(root.path()).template, Template::Open);
}

#[test]
fn a_log_line_is_not_a_read_of_the_whole_log() {
    // #7073: the cap counts the lines once and keeps the count, full or not.
    let root = tempfile::tempdir().expect("a folder");
    let path = log_file(root.path(), CHAT).expect("a log file");
    std::fs::create_dir_all(path.parent().expect("its folder")).expect("its folder");
    std::fs::write(&path, "{}\n".repeat(MOST_LINES - 2)).expect("a log near full");
    let before = counted_whole();
    for _ in 0..4 {
        write_line(root.path(), CHAT, Family::GuardsGit, "A3", None);
    }
    assert_eq!(counted_whole() - before, 1, "counted once");
    let kept = std::fs::read_to_string(&path).expect("the log");
    assert_eq!(kept.lines().count(), MOST_LINES);
    // A log taken away is counted again, from nothing.
    std::fs::remove_file(&path).expect("the log gone");
    write_line(root.path(), CHAT, Family::GuardsGit, "A3", None);
    assert_eq!(read_log(root.path(), CHAT).len(), 1);
}

#[test]
fn a_press_with_a_seal_key_another_program_made_says_how_to_recover_and_then_accepts() {
    // #7073, #7714: Accept says when this happens without claiming who made the item, and how
    // to recover by hand; once the item is gone, the next press makes a key and accepts.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    let refused = press(root.path(), &digest_of(root.path()));
    crate::secrets::seal::set_foreign_for_this_test(false);
    let why = refused.expect_err("nothing accepted").to_string();
    assert_eq!(why, crate::secrets::seal::FOREIGN_KEY);
    assert!(
        why.contains("This happens after an update that macOS treats as a different app"),
        "{why}"
    );
    assert!(
        !why.contains("earlier build"),
        "never claims the origin: {why}"
    );
    assert!(
        why.contains("security delete-generic-password -s purlis-seal"),
        "{why}"
    );
    press(root.path(), &digest_of(root.path())).expect("accepted once the item is gone");
    assert!(!broken(root.path()));
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
}

// ---- lineage and protected branches: an allowance and a list read as `[policy]` is ---------------

/// `text` written as the plane's committed file and committed.
fn commit_file(root: &Path, text: &str) {
    std::fs::write(root.join("charter.toml"), text).expect("written");
    for args in [
        &["add", "charter.toml"][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "x",
        ],
    ] {
        let ran = git_in(root, args);
        assert!(ran.status.success(), "{args:?}: {ran:?}");
    }
}

const DENY_LINEAGE: &str = "[policy]\nlineage = \"deny\"\n";

#[test]
fn lineage_is_on_where_nothing_is_set_and_guards_forge_has_no_say_in_it() {
    let _home = OwnConfigHome::new();
    for text in [
        "",
        "[policy]\n",
        "[policy]\nlineage = \"allow\"\n",
        "[policy]\nguards.forge = \"off\"\n",
        "[policy]\nguards.forge = \"log\"\n",
        "[policy]\nguards.forge = \"enforce\"\n",
        "[policy]\ntemplate = \"strict\"\n",
    ] {
        let root = a_committed_plane(text);
        assert!(lineage_in_force(root.path()), "{text}");
    }
}

#[test]
fn tightening_guards_forge_from_off_to_log_does_not_turn_lineage_on() {
    let _home = OwnConfigHome::new();
    for forge in ["off", "log", "enforce"] {
        let root = a_committed_plane(&format!("{DENY_LINEAGE}guards.forge = \"{forge}\"\n"));
        assert!(!lineage_in_force(root.path()), "{forge}");
    }
}

#[test]
fn a_commit_that_denies_lineage_takes_effect_at_once() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane("[policy]\n");
    assert!(lineage_in_force(root.path()));
    commit_file(root.path(), DENY_LINEAGE);
    assert!(!lineage_in_force(root.path()));
    assert!(!standing(root.path()).lineage_waiting);
}

#[test]
fn a_commit_that_allows_lineage_again_waits_for_the_press() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(DENY_LINEAGE);
    assert!(!lineage_in_force(root.path()));
    commit_file(root.path(), "[policy]\n");
    assert!(!lineage_in_force(root.path()), "still denied until pressed");
    let waiting = standing(root.path());
    assert!(waiting.lineage_waiting);
    press(root.path(), &waiting.digest).expect("pressed");
    assert!(lineage_in_force(root.path()));
    assert!(!standing(root.path()).lineage_waiting);
}

#[test]
fn a_lock_holds_lineage_off_and_a_refused_lock_file_does_too() {
    let policy = std::path::Path::new("/etc/purlis/policy.json");
    let held = Locks::parse(r#"{"sandbox": {"lineage": false}}"#, policy);
    assert!(held.forbids_lineage());
    assert!(!Resolved::of(&said(""), &held).lineage());
    assert!(!Locks::none().forbids_lineage());
    assert!(Resolved::of(&said(""), &Locks::none()).lineage());
    for text in [r#"{"sandbox": {"lineage": true}}"#, r#"{"sandbox": {}}"#] {
        assert!(!Locks::parse(text, policy).forbids_lineage(), "{text}");
    }
    let refused = Locks::parse("not json", policy);
    assert!(refused.forbids_lineage());
    assert!(!Resolved::of(&said(""), &refused).lineage());
}

#[test]
fn a_value_purlis_cannot_read_means_no_lineage() {
    for text in [
        "[policy]\nlineage = \"maybe\"\n",
        "[policy]\nlineage = true\n",
        "[policy]\nlineage = 1\n",
        "[policy]\nlineage = [\"allow\"]\n",
        "[policy]\ntemplate = \"loose\"\n",
        "[policy]\nnot_a_family = \"off\"\n",
        "policy = [unclosed\n",
        "policy = 3\n",
    ] {
        let settings = said(text);
        assert!(settings.lineage_denied, "{text}");
        assert!(!Resolved::of(&settings, &Locks::none()).lineage(), "{text}");
    }
    assert!(!said("[policy]\nlineage = \"allow\"\n").lineage_denied);
}

#[test]
fn a_head_git_cannot_read_means_no_lineage_and_is_never_kept() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane("[policy]\n");
    assert!(lineage_in_force(root.path()));
    std::fs::remove_file(the_files_blob(root.path())).expect("the blob gone");
    assert!(!lineage_in_force(root.path()));
}

#[test]
fn the_local_file_can_deny_lineage_and_never_allow_it() {
    let denied = plane(DENY_LINEAGE, Some("[policy]\nlineage = \"allow\"\n"));
    assert!(Settings::read(denied.path()).lineage_denied);
    let local = plane("[policy]\n", Some(DENY_LINEAGE));
    assert!(Settings::read(local.path()).lineage_denied);
}

#[test]
fn lineage_in_the_working_tree_alone_changes_nothing() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(DENY_LINEAGE);
    assert!(!lineage_in_force(root.path()));
    std::fs::write(root.path().join("charter.toml"), "[policy]\n").expect("edited, not committed");
    assert!(!lineage_in_force(root.path()));
}

const PROTECT: &str = "[risk]\nprotected_branches = [\"m75-policy\", \"phase-1-proven\"]\n";

#[test]
fn the_protected_branches_are_read_from_head() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(PROTECT);
    assert_eq!(
        protected_branches_in_force(root.path()),
        ["m75-policy", "phase-1-proven"]
    );
}

#[test]
fn a_protected_list_edited_or_broken_in_the_working_tree_loses_nothing() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(PROTECT);
    assert_eq!(protected_branches_in_force(root.path()).len(), 2);
    for edited in [
        "",
        "[risk]\nprotected_branches = [\"x\"]\n",
        "risk = [unclosed\n",
    ] {
        std::fs::write(root.path().join("charter.toml"), edited).expect("not committed");
        assert_eq!(
            protected_branches_in_force(root.path()),
            ["m75-policy", "phase-1-proven"],
            "{edited}"
        );
    }
}

#[test]
fn a_commit_that_drops_a_protection_waits_for_the_press_and_one_that_adds_holds_at_once() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(PROTECT);
    assert_eq!(protected_branches_in_force(root.path()).len(), 2);
    // A name added: at once.
    commit_file(
        root.path(),
        "[risk]\nprotected_branches = [\"m75-policy\", \"phase-1-proven\", \"lts/*\"]\n",
    );
    assert_eq!(
        protected_branches_in_force(root.path()),
        ["lts/*", "m75-policy", "phase-1-proven"]
    );
    // A name dropped: held until the person presses.
    commit_file(
        root.path(),
        "[risk]\nprotected_branches = [\"m75-policy\"]\n",
    );
    assert_eq!(protected_branches_in_force(root.path()).len(), 3);
    let waiting = standing(root.path());
    assert_eq!(waiting.protection_lifted, ["lts/*", "phase-1-proven"]);
    press(root.path(), &waiting.digest).expect("pressed");
    assert_eq!(protected_branches_in_force(root.path()), ["m75-policy"]);
}

#[test]
fn a_head_git_cannot_read_keeps_the_accepted_protected_list() {
    let _home = OwnConfigHome::new();
    let root = a_committed_plane(PROTECT);
    assert_eq!(protected_branches_in_force(root.path()).len(), 2);
    std::fs::remove_file(the_files_blob(root.path())).expect("the blob gone");
    assert_eq!(
        protected_branches_in_force(root.path()),
        ["m75-policy", "phase-1-proven"]
    );
}

#[test]
fn the_protected_list_and_the_lineage_denial_are_kept_and_read_back() {
    let settings = said(&format!("{DENY_LINEAGE}{PROTECT}"));
    let resolved = Resolved::of(&settings, &Locks::none());
    let kept = resolved.as_kept_for_test();
    let back = Resolved::of_kept_for_test(&kept);
    assert!(!resolved.lineage());
    assert!(!back.lineage());
    assert_eq!(back.protected_branches(), ["m75-policy", "phase-1-proven"]);
    // An acceptance from before either key existed reads as allowed, with nothing protected.
    let old = Resolved::of(&said(""), &Locks::none()).as_kept_for_test();
    let read = Resolved::of_kept_for_test(&old);
    assert!(read.lineage());
    assert!(read.protected_branches().is_empty());
}

// ---- an update's foreign seal key: one press makes a new key and accepts (#7714) ----------------

/// A plane whose acceptance of `committed` was pressed and sealed with the key of now.
fn accepted_plane(committed: &str) -> tempfile::TempDir {
    let root = plane(committed, None);
    press(root.path(), &digest_of(root.path())).expect("accepted");
    root
}

#[test]
fn a_press_with_an_updates_foreign_key_makes_a_new_key_and_accepts_what_was_shown() {
    // #7714: after an update macOS takes for another program, the old build owns the seal's
    // key. The one press replaces it and seals this plane's acceptance of what was shown.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    let shown = standing(root.path());
    assert!(shown.key_foreign, "the window is told the key is foreign");
    assert!(broken(root.path()));
    press_with_new_key(root.path(), &shown.digest).expect("a new key, and accepted");
    assert!(!crate::secrets::seal::key_is_foreign());
    assert!(matches!(
        crate::sandbox::local::policy_accepted_record(root.path()),
        crate::sandbox::local::PolicyAccepted::Held(_)
    ));
    assert_eq!(in_force(root.path()).mode(Family::GuardsGit), Mode::Off);
    let after = standing(root.path());
    assert!(!after.key_foreign && !after.unsealed && !after.new_key);
}

#[test]
fn the_new_key_press_still_accepts_only_what_was_shown() {
    // The digest rule stays: a file that changed since it was shown accepts nothing, and no
    // key is replaced for it.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    let refused = press_with_new_key(root.path(), "not what was shown");
    assert!(refused.is_err());
    assert!(crate::secrets::seal::key_is_foreign(), "no key replaced");
    crate::secrets::seal::set_foreign_for_this_test(false);
}

#[test]
fn another_plane_sealed_with_the_old_key_reads_as_accept_again_with_why() {
    // #7714: the other planes' seals no longer verify under the new key. Each reads as needing
    // acceptance again because purlis made a new key, never as a record edited by something.
    let _home = OwnConfigHome::new();
    let here = accepted_plane("[policy]\ntemplate = \"open\"\n");
    let other = accepted_plane("[policy]\ntemplate = \"open\"\n");
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    let there = standing(other.path());
    assert!(
        broken(other.path()),
        "enforced as strictly as any broken record"
    );
    assert!(
        there.unsealed && there.new_key,
        "said as a new key: {there:?}"
    );
    // Accepted again, it holds, and the reason is gone.
    press(other.path(), &digest_of(other.path())).expect("accepted again");
    let there = standing(other.path());
    assert!(!there.unsealed && !there.new_key);
}

#[test]
fn a_record_edited_after_the_new_key_still_reads_broken_with_the_tamper_words() {
    // #7714: tamper detection is not weakened. A record of this plane edited after the new key
    // sealed it reads broken, and is never said to be the new key's.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(root.path(), &digest_of(root.path())).expect("a new key");
    let file = crate::sandbox::local::path(root.path());
    let text = std::fs::read_to_string(&file).expect("the record");
    let edited = text.replacen("\"asks\": \"off\"", "\"asks\": \"log\"", 1);
    assert_ne!(edited, text, "{text}");
    std::fs::write(&file, edited).expect("edited");
    let now = standing(root.path());
    assert!(broken(root.path()));
    assert!(now.unsealed && !now.new_key, "the tamper words: {now:?}");
}

#[test]
fn a_new_key_press_macos_refuses_says_the_manual_step_and_accepts_nothing() {
    // #7714: where macOS does not let purlis delete the old build's items, the press says so
    // plainly and names the manual step.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    crate::secrets::seal::set_foreign_for_this_test(true);
    crate::secrets::seal::set_renew_refused_for_this_test(true);
    let why = press_with_new_key(root.path(), &digest_of(root.path()))
        .expect_err("nothing accepted")
        .to_string();
    crate::secrets::seal::set_renew_refused_for_this_test(false);
    assert!(
        why.contains("macOS did not let purlis replace its key, so nothing was accepted"),
        "{why}"
    );
    assert!(
        why.contains("security delete-generic-password -s purlis-seal"),
        "{why}"
    );
    assert!(crate::secrets::seal::key_is_foreign());
    assert!(broken(root.path()));
    // The press can be made again, and this time macOS allows it.
    press_with_new_key(root.path(), &digest_of(root.path())).expect("pressed again");
    assert!(!broken(root.path()));
}

#[test]
fn the_new_key_press_checks_what_was_shown_again_after_macos_prompt_returns() {
    // #7714, ruling 5: the file can change while macOS's prompt is open. The press then accepts
    // nothing, keeps the new key, and a press on what is shown now accepts.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n", None);
    let shown = digest_of(root.path());
    crate::secrets::seal::set_foreign_for_this_test(true);
    let file = root.path().join("charter.toml");
    crate::secrets::seal::set_during_prompt_for_this_test(Box::new(move || {
        std::fs::write(file, "[policy]\ntemplate = \"team\"\n").expect("changed meanwhile");
    }));
    let why = press_with_new_key(root.path(), &shown)
        .expect_err("nothing accepted")
        .to_string();
    assert!(why.contains("changed since it was shown"), "{why}");
    assert!(
        !crate::secrets::seal::key_is_foreign(),
        "the new key is kept"
    );
    assert!(!matches!(
        crate::sandbox::local::policy_accepted_record(root.path()),
        crate::sandbox::local::PolicyAccepted::Held(_)
    ));
    press_with_new_key(root.path(), &digest_of(root.path())).expect("accepted as shown now");
    assert_eq!(
        crate::sandbox::local::policy_accepted_record(root.path()),
        crate::sandbox::local::PolicyAccepted::Held(committed(root.path()).as_kept())
    );
}

#[test]
fn after_the_new_key_press_no_other_project_that_had_an_acceptance_reads_never() {
    // #7714, ruling 1: every other project that had an acceptance stays broken, so no accepted
    // deny drops to the shipped default; one whose record was already gone too. A project that
    // never had one is left as it was, and the press accepts its own project alone.
    use crate::sandbox::local::{PolicyAccepted, policy_accepted_record};
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let kept = accepted_plane("[policy]\ntemplate = \"strict\"\n");
    let gone = accepted_plane("[policy]\ntemplate = \"strict\"\n");
    let never = plane("[policy]\ntemplate = \"open\"\n", None);
    std::fs::remove_file(crate::sandbox::local::path(gone.path())).expect("record gone");
    assert_eq!(policy_accepted_record(gone.path()), PolicyAccepted::Broken);
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    for other in [&kept, &gone] {
        assert_eq!(policy_accepted_record(other.path()), PolicyAccepted::Broken);
        assert!(standing(other.path()).new_key);
    }
    assert_eq!(policy_accepted_record(never.path()), PolicyAccepted::Never);
    assert!(!standing(never.path()).new_key);
    assert!(matches!(
        policy_accepted_record(here.path()),
        PolicyAccepted::Held(_)
    ));
}

#[test]
fn a_new_key_press_where_the_key_is_the_apps_own_replaces_nothing() {
    // The press makes a new key only where the old one is another program's: anywhere else it
    // is Accept, and no other plane's seal is broken by it.
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let other = accepted_plane("[policy]\ntemplate = \"open\"\n");
    press_with_new_key(here.path(), &digest_of(here.path())).expect("accepted");
    assert!(!broken(here.path()) && !broken(other.path()));
    assert!(!standing(other.path()).new_key);
}

#[test]
fn a_list_of_replaced_planes_whose_seal_does_not_hold_gives_no_new_key_words() {
    // #7714, F3: the list is sealed with the new key. One edited, or put there by another
    // program, gives no plane the new key's words; it only ever keeps a plane broken.
    use crate::sandbox::local::{PolicyAccepted, anchor_key, policy_accepted_record};
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let other = accepted_plane("[policy]\ntemplate = \"strict\"\n");
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    assert!(standing(other.path()).new_key, "the list holds");
    crate::secrets::seal::set_replaced_for_this_test(&format!(
        "{}\n{}",
        "00".repeat(32),
        anchor_key(other.path())
    ));
    assert!(
        !standing(other.path()).new_key,
        "a list that does not hold says nothing"
    );
    assert_eq!(policy_accepted_record(other.path()), PolicyAccepted::Broken);
}

#[test]
fn a_project_accepted_again_after_the_new_key_is_taken_off_the_list() {
    // #7714, F3: the entry goes when the project is accepted again, and the item with its last
    // entry; a record that breaks after that gets the tamper words.
    use crate::sandbox::local::anchor_key;
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let other = accepted_plane("[policy]\ntemplate = \"open\"\n");
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    assert!(crate::secrets::seal::replaced().holds_for(&anchor_key(other.path())));
    press(other.path(), &digest_of(other.path())).expect("accepted again");
    assert_eq!(
        crate::secrets::seal::replaced(),
        crate::secrets::seal::Replaced::default(),
        "its last entry gone, the item went with it"
    );
    let file = crate::sandbox::local::path(other.path());
    let text = std::fs::read_to_string(&file).expect("the record");
    let edited = text.replacen("\"asks\": \"off\"", "\"asks\": \"log\"", 1);
    assert_ne!(edited, text, "{text}");
    std::fs::write(&file, edited).expect("edited");
    let now = standing(other.path());
    assert!(broken(other.path()));
    assert!(now.unsealed && !now.new_key, "the tamper words: {now:?}");
}

#[test]
fn a_second_new_key_adds_to_the_planes_the_first_listed() {
    // #7714, F3: a plane the first replacement listed and not accepted since has no anchor left
    // for the second to find; the list it kept carries it.
    use crate::sandbox::local::anchor_key;
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let first = accepted_plane("[policy]\ntemplate = \"open\"\n");
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a first new key");
    crate::secrets::seal::set_foreign_for_this_test(true);
    let again = plane("[policy]\ntemplate = \"team\"\n", None);
    press_with_new_key(again.path(), &digest_of(again.path())).expect("a second new key");
    let listed = crate::secrets::seal::replaced();
    assert!(listed.holds_for(&anchor_key(first.path())), "{listed:?}");
    assert!(listed.holds_for(&anchor_key(here.path())), "{listed:?}");
    assert!(standing(first.path()).new_key && standing(here.path()).new_key);
}

// ---- the nevers after a new key (#7734's review, F3) -------------------------------------------

/// The pair `steward` to `devops` said never to in the plane at `root`, sealed with the key of
/// now.
fn a_never(root: &std::path::Path) {
    crate::dispatchnever::add(root, "steward", "devops").expect("a never kept");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Read(vec![("steward".to_owned(), "devops".to_owned())])
    );
}

#[test]
fn after_a_new_key_an_untouched_nevers_file_says_the_new_key_never_restore_that_file() {
    // F3: the old key's anchor of the nevers went with it. The file is as purlis wrote it, so
    // "Restore that file" cannot be done: the words say purlis made a new key, and to review it.
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    for other in [
        accepted_plane("[policy]\ntemplate = \"open\"\n"),
        plane("[policy]\ntemplate = \"open\"\n", None),
    ] {
        a_never(other.path());
        crate::secrets::seal::set_foreign_for_this_test(true);
        press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
        assert_eq!(
            crate::dispatchnever::read(other.path()),
            crate::dispatchnever::Nevers::Unread
        );
        let said = crate::dispatchnever::unread_said(other.path());
        assert!(said.contains("purlis made a new seal key"), "{said}");
        assert!(!said.contains("Restore that file"), "{said}");
        assert!(standing(other.path()).nevers_new_key);
    }
}

#[test]
fn after_a_new_key_a_wiped_nevers_file_is_never_read_as_no_nevers() {
    // F3: a project whose nevers the old key anchored, and whose file is gone now. Read as no
    // nevers, the person's refusals would drop silently; it reads as unread until they accept.
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let other = plane("[policy]\ntemplate = \"open\"\n", None);
    a_never(other.path());
    std::fs::remove_file(crate::dispatchnever::path(other.path())).expect("wiped");
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    assert_eq!(
        crate::dispatchnever::read(other.path()),
        crate::dispatchnever::Nevers::Unread
    );
}

#[test]
fn accepting_a_project_again_after_a_new_key_seals_its_nevers_as_they_stand() {
    // F3: the person's Accept on that project seals the nevers the file holds with the new key
    // (they only tighten), and the project leaves the list.
    let _home = OwnConfigHome::new();
    let here = plane("[policy]\ntemplate = \"open\"\n", None);
    let other = accepted_plane("[policy]\ntemplate = \"open\"\n");
    a_never(other.path());
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    press(other.path(), &digest_of(other.path())).expect("accepted again");
    assert_eq!(
        crate::dispatchnever::read(other.path()),
        crate::dispatchnever::Nevers::Read(vec![("steward".to_owned(), "devops".to_owned())])
    );
    assert!(!standing(other.path()).nevers_new_key);
}
