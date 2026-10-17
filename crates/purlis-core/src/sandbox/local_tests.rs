// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What one machine keeps about a project's sandbox: the one-time offer (ruling V21 1) and the
//! opt-out count (V12, ruling V78 d). Every expected answer is written out.

use super::*;
use crate::sandbox::{Policy, Preset};

fn a_project(charter_toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join(crate::plane::MANIFEST), charter_toml).expect("charter.toml");
    dir
}

fn charter_toml(root: &Path) -> String {
    std::fs::read_to_string(root.join(crate::plane::MANIFEST)).expect("charter.toml")
}

// -------------------------------------------------------------------------------------
// The one-time offer
// -------------------------------------------------------------------------------------

#[test]
fn an_existing_project_without_the_sandbox_is_offered_it() {
    let project = a_project("schema = 1\n\n[memory]\nshare = \"local\"\n");
    assert!(offer_due(project.path()));
}

#[test]
fn a_project_charter_made_is_never_offered_what_it_already_has() {
    let project = a_project(&crate::scaffold::planefile::render("github", "acme", None));
    assert!(!offer_due(project.path()));
}

#[test]
fn a_directory_that_is_no_project_is_offered_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    assert!(!offer_due(dir.path()));
}

#[test]
fn taking_the_offer_turns_the_sandbox_on_and_keeps_every_line_the_operator_wrote() {
    let before = "schema = 1\n# the forge we use\n[[forge]]\nkind = \"github\"\n";
    let project = a_project(before);

    answer(project.path(), Answer::TurnOn).expect("answered");

    let after = charter_toml(project.path());
    assert_eq!(
        after,
        format!(
            "{before}\n[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\", \"toolchains\"]\n"
        )
    );
    assert_eq!(
        Plane::read(project.path()).said().policy,
        Some(Policy {
            egress: Preset::DEFAULT.to_vec(),
            hosts: vec![],
            certificate_checks: true,
            personas: Default::default(),
        })
    );
    assert!(!offer_due(project.path()), "asked once");
}

/// #1405: a `[[forge]]` host the sandbox does not let through is said once the sandbox is on,
/// and never stops the offer from turning it on.
#[test]
fn taking_the_offer_turns_the_sandbox_on_beside_a_forge_host_it_does_not_let_through() {
    let project =
        a_project("schema = 1\n[[forge]]\nkind = \"gitlab\"\nhost = \"169.254.169.254\"\n");

    answer(project.path(), Answer::TurnOn).expect("answered");

    let said = Plane::read(project.path()).said();
    assert!(said.policy.is_some());
    assert!(
        said.refused
            .iter()
            .any(|refusal| matches!(refusal, crate::sandbox::Refusal::ForgeHost(..))),
        "{:?}",
        said.refused
    );
}

#[test]
fn a_sandbox_table_without_a_mode_gains_the_mode_and_keeps_its_egress() {
    let project = a_project("schema = 1\n\n[sandbox]\negress = [\"forge\"]\n");

    answer(project.path(), Answer::TurnOn).expect("answered");

    assert_eq!(
        Plane::read(project.path()).said().policy,
        Some(Policy {
            egress: vec![Preset::Forge],
            hosts: vec![],
            certificate_checks: true,
            personas: Default::default(),
        })
    );
}

#[test]
fn a_sandbox_charter_cannot_edit_safely_is_refused_and_nothing_is_written() {
    let before = "schema = 1\nsandbox = { egress = [\"forge\"] }\n";
    let project = a_project(before);

    let refused = answer(project.path(), Answer::TurnOn).expect_err("refused");

    assert!(refused.to_string().contains("mode = \"on\""), "{refused}");
    assert_eq!(charter_toml(project.path()), before);
    assert!(offer_due(project.path()), "still unanswered");
}

/// A `[sandbox]` or `mode =` line inside a multi-line string is not the table. An edit that
/// lands there leaves the sandbox off, so it is refused, nothing is written, and the offer is
/// still due — never recorded as taken with the sandbox off.
#[test]
fn an_edit_that_would_land_in_a_string_and_leave_the_sandbox_off_is_refused() {
    for before in [
        "schema = 1\nnote = \"\"\"\n[sandbox]\n\"\"\"\nsandbox = { egress = [\"forge\"] }\n",
        "schema = 1\n[sandbox]\negress = \"\"\"\nmode = 1\n\"\"\"\n",
    ] {
        let project = a_project(before);

        let refused = answer(project.path(), Answer::TurnOn).expect_err(before);

        assert!(refused.to_string().contains("by hand"), "{refused}");
        assert_eq!(charter_toml(project.path()), before, "nothing written");
        assert_eq!(Plane::read(project.path()).said().policy, None);
        assert!(offer_due(project.path()), "still unanswered: {before}");
    }
}

/// A `charter.toml` charter cannot read refuses every chat (fail closed), so it is offered
/// nothing, and taking an offer never writes into one.
#[test]
fn a_charter_toml_charter_cannot_read_is_never_offered_or_written() {
    let before = "schema = 1\n[sandbox\nmode = \"on\"\n";
    let project = a_project(before);

    assert!(!offer_due(project.path()));
    assert!(answer(project.path(), Answer::TurnOn).is_err());
    assert_eq!(charter_toml(project.path()), before, "nothing written");
}

/// A `charter.toml` that is a link refuses every chat, so it is never offered the sandbox, and
/// taking the offer never writes through it to the file it names.
#[cfg(unix)]
#[test]
fn a_charter_toml_that_is_a_link_is_never_offered_or_written_through() {
    let project = tempfile::tempdir().expect("a project");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let real = elsewhere.path().join("charter.toml");
    std::fs::write(&real, "schema = 1\n").expect("the target");
    std::os::unix::fs::symlink(&real, project.path().join(crate::plane::MANIFEST))
        .expect("the link");

    assert!(!offer_due(project.path()));
    assert!(answer(project.path(), Answer::TurnOn).is_err());
    assert_eq!(
        std::fs::read_to_string(&real).expect("the target"),
        "schema = 1\n",
        "nothing written through the link"
    );
    assert!(
        std::fs::symlink_metadata(project.path().join(crate::plane::MANIFEST))
            .expect("still there")
            .file_type()
            .is_symlink(),
        "the link is not replaced"
    );
}

/// One over the cap the sandbox reads is unreadable too: never offered, never written.
#[test]
fn a_charter_toml_over_the_cap_is_never_offered_or_written() {
    let before = format!(
        "schema = 1\n# {}\n",
        "x".repeat(crate::sandbox::PLANE_FILE_MAX)
    );
    let project = a_project(&before);

    assert!(!offer_due(project.path()));
    assert!(answer(project.path(), Answer::TurnOn).is_err());
    assert_eq!(charter_toml(project.path()), before, "nothing written");
}

#[test]
fn declining_the_offer_changes_nothing_in_the_project_and_is_never_asked_again() {
    let before = "schema = 1\n";
    let project = a_project(before);

    answer(project.path(), Answer::KeepItOff).expect("answered");

    assert_eq!(charter_toml(project.path()), before, "nothing flips on");
    assert_eq!(Plane::read(project.path()).said().policy, None);
    assert!(!offer_due(project.path()), "asked once");
}

#[test]
fn the_answer_is_kept_where_a_chat_cannot_write_it() {
    let project = a_project("schema = 1\n");
    answer(project.path(), Answer::KeepItOff).expect("answered");

    let kept = path(project.path());
    assert!(kept.is_file(), "{}", kept.display());
    // purlis's folder in the config home (ADR 0093): never the project, and a sandboxed chat
    // never writes it.
    assert!(!kept.starts_with(project.path()));
    assert!(kept.starts_with(crate::machine::dir(&config_home().expect("a config home"))));
}

// -------------------------------------------------------------------------------------
// The opt-out count
// -------------------------------------------------------------------------------------

#[test]
fn a_project_with_no_chats_counted_has_an_empty_tally() {
    let project = a_project("schema = 1\n");
    assert_eq!(tally(project.path()), Tally::default());
    assert_eq!(
        tally(project.path()).said(),
        "no chat has started under this project's sandbox on this machine yet"
    );
}

#[test]
fn each_new_chat_is_counted_by_how_it_started() {
    let project = a_project("schema = 1\n");
    for started in [
        Started::Sandboxed,
        Started::Sandboxed,
        Started::Sandboxed,
        Started::OptedOut,
        Started::NoBackend,
    ] {
        count(project.path(), started).expect("counted");
    }

    assert_eq!(
        tally(project.path()),
        Tally {
            sandboxed: 3,
            opted_out: 1,
            no_backend: 1,
        }
    );
}

#[test]
fn the_rate_is_the_opt_outs_among_the_chats_a_person_could_have_sandboxed() {
    let tally = Tally {
        sandboxed: 19,
        opted_out: 1,
        no_backend: 0,
    };
    assert_eq!(tally.rate_percent(), Some(5));
    assert_eq!(
        tally.said(),
        "1 of 20 chats started without the sandbox on this machine (5%); the bar is under 10%"
    );
}

#[test]
fn chats_on_a_machine_with_no_backend_are_said_apart_and_never_counted_as_a_choice() {
    let tally = Tally {
        sandboxed: 0,
        opted_out: 0,
        no_backend: 4,
    };
    assert_eq!(tally.rate_percent(), None);
    assert_eq!(
        tally.said(),
        "4 chats started without the sandbox because this machine has no sandbox backend"
    );
}

#[test]
fn a_rate_rounds_up_so_it_never_reads_under_the_bar_when_it_is_not() {
    let tally = Tally {
        sandboxed: 90,
        opted_out: 10,
        no_backend: 2,
    };
    assert_eq!(tally.rate_percent(), Some(10));
    let tally = Tally {
        sandboxed: 2,
        opted_out: 1,
        no_backend: 0,
    };
    assert_eq!(tally.rate_percent(), Some(34));
    assert_eq!(
        tally.said(),
        "1 of 3 chats started without the sandbox on this machine (34%); the bar is under 10%"
    );
}

#[test]
fn a_tally_charter_cannot_read_reads_as_empty_and_the_next_count_starts_it_again() {
    let project = a_project("schema = 1\n");
    let file = path(project.path());
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the directory");
    std::fs::write(&file, "not json").expect("written");

    assert_eq!(tally(project.path()), Tally::default());
    count(project.path(), Started::OptedOut).expect("counted");
    assert_eq!(tally(project.path()).opted_out, 1);
}

#[test]
fn counting_keeps_the_offers_answer_and_answering_keeps_the_count() {
    let project = a_project("schema = 1\n");
    count(project.path(), Started::Sandboxed).expect("counted");
    answer(project.path(), Answer::KeepItOff).expect("answered");
    count(project.path(), Started::OptedOut).expect("counted");

    assert!(!offer_due(project.path()));
    assert_eq!(
        tally(project.path()),
        Tally {
            sandboxed: 1,
            opted_out: 1,
            no_backend: 0,
        }
    );
}

// -------------------------------------------------------------------------------------
// The project's hosts: one Notice per change, on each machine (#1341)
// -------------------------------------------------------------------------------------

const HOSTS: &str = "[sandbox]\nmode = \"on\"\nhosts = [\"10.100.39.145:6443\", \"a.example\"]\n";

fn changed(added: &[&str], removed: &[&str], now: &[&str]) -> Option<HostsChange> {
    let owned = |list: &[&str]| list.iter().map(|one| (*one).to_owned()).collect();
    Some(HostsChange {
        added: owned(added),
        removed: owned(removed),
        now: owned(now),
    })
}

#[test]
fn a_teammate_is_told_once_of_the_hosts_a_project_already_has() {
    let project = a_project(HOSTS);
    let seen = hosts_changed(project.path());
    assert_eq!(
        seen,
        changed(
            &["10.100.39.145:6443", "a.example"],
            &[],
            &["10.100.39.145:6443", "a.example"]
        )
    );
    acknowledge_hosts(project.path(), &seen.expect("a change").now).expect("kept");
    assert_eq!(hosts_changed(project.path()), None, "told once");
}

#[test]
fn a_change_names_what_was_added_and_what_was_taken_away() {
    let project = a_project(HOSTS);
    acknowledge_hosts(
        project.path(),
        &["10.100.39.145:6443".to_owned(), "a.example".to_owned()],
    )
    .expect("kept");
    std::fs::write(
        project.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\", \"*.b.example\"]\n",
    )
    .expect("a teammate's push");
    assert_eq!(
        hosts_changed(project.path()),
        changed(
            &["*.b.example"],
            &["10.100.39.145:6443"],
            &["a.example", "*.b.example"]
        )
    );
}

#[test]
fn what_was_acknowledged_is_what_was_shown_so_a_later_change_is_still_told() {
    let project = a_project(HOSTS);
    // Shown before the next push, and acknowledged after it.
    acknowledge_hosts(project.path(), &["a.example".to_owned()]).expect("kept");
    assert_eq!(
        hosts_changed(project.path()),
        changed(
            &["10.100.39.145:6443"],
            &[],
            &["10.100.39.145:6443", "a.example"]
        )
    );
}

#[test]
fn a_project_with_no_hosts_or_no_sandbox_tells_nothing() {
    assert_eq!(
        hosts_changed(a_project("[sandbox]\nmode = \"on\"\n").path()),
        None
    );
    // Hosts in a project whose chats are not sandboxed reach nothing yet: told once it is on.
    assert_eq!(
        hosts_changed(a_project("[sandbox]\nhosts = [\"a.example\"]\n").path()),
        None
    );
}

/// Fold-in a of the #1362 review: an older purlis told a persona's hosts with the project's, as
/// `<host> for <persona> chats`. Such an entry is no project host taken away.
#[test]
fn a_persona_host_an_older_purlis_told_is_not_said_to_be_taken_away() {
    let project = a_project(HOSTS);
    acknowledge_hosts(
        project.path(),
        &[
            "10.100.39.145:6443".to_owned(),
            "a.example".to_owned(),
            "10.0.0.5:6443 for devops chats".to_owned(),
        ],
    )
    .expect("kept");
    assert_eq!(hosts_changed(project.path()), None);
}

#[test]
fn reordering_the_hosts_is_no_change() {
    let project = a_project(HOSTS);
    acknowledge_hosts(
        project.path(),
        &["a.example".to_owned(), "10.100.39.145:6443".to_owned()],
    )
    .expect("kept");
    assert_eq!(hosts_changed(project.path()), None);
}

#[test]
fn a_vault_allowed_for_a_persona_is_kept_here_once_and_revoked_with_its_record() {
    let plane = tempfile::tempdir().expect("a directory");
    let root = plane.path();
    assert_eq!(granted_vaults(root), Vec::new());
    grant_vault(root, "devops", "steward").expect("granted");
    grant_vault(root, "devops", "steward").expect("granted again");
    grant_vault(root, "devops", "reviewer").expect("another persona");
    let steward = VaultGrant {
        vault: "devops".into(),
        persona: "steward".into(),
    };
    assert_eq!(steward.target(), "devops for steward");
    assert_eq!(
        granted_vaults(root),
        vec![
            steward.clone(),
            VaultGrant {
                vault: "devops".into(),
                persona: "reviewer".into(),
            },
        ]
    );
    record_made(
        root,
        Made {
            what: VAULT.to_owned(),
            target: steward.target(),
            level: "you".to_owned(),
            at: 7,
            chat: Some("steward 1".to_owned()),
        },
    )
    .expect("recorded");
    revoke_vault(root, "devops", "steward").expect("revoked");
    assert_eq!(
        granted_vaults(root),
        vec![VaultGrant {
            vault: "devops".into(),
            persona: "reviewer".into(),
        }]
    );
    assert_eq!(made(root), Vec::new(), "its record goes with it");
    // It is this machine's file, in its config home, never the project (ADR 0093), and never the
    // vault registry.
    assert!(!path(root).starts_with(root));
    assert!(!root.join("vaults.json").exists());
}

// -------------------------------------------------------------------------------------
// The project's Internet access presets: one Notice per change, on each machine (#1385)
// -------------------------------------------------------------------------------------

fn presets(
    text: &str,
    locks: &super::super::policy::Locks,
    seen: Option<&[&str]>,
) -> Option<PresetsChange> {
    let seen: Option<Vec<String>> =
        seen.map(|seen| seen.iter().map(|one| (*one).to_owned()).collect());
    presets_change(&Plane::of(Some(text)), locks, seen.as_deref())
}

fn no_locks() -> super::super::policy::Locks {
    super::super::policy::Locks::none()
}

#[test]
fn a_project_on_the_default_presets_tells_nothing_on_first_sight() {
    assert_eq!(
        presets("[sandbox]\nmode = \"on\"\n", &no_locks(), None),
        None
    );
    assert_eq!(
        presets(
            "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\", \"toolchains\"]\n",
            &no_locks(),
            None
        ),
        None
    );
}

/// R1 (2026-10-13): certificate checks are among the defaults a machine has seen, so a default
/// project tells nothing; a project that turned them off is told once, as a narrowed one is;
/// and a machine that recorded what it saw before they were on by default is told once that
/// they are, with what that widens.
#[test]
fn certificate_checks_are_a_default_and_a_change_from_what_was_seen_is_told() {
    const ALL: [&str; 4] = [
        "model-providers",
        "forge",
        "toolchains",
        "certificate-checks",
    ];
    for on in [
        "[sandbox]\nmode = \"on\"\n",
        "[sandbox]\nmode = \"on\"\ncertificate-checks = true\n",
    ] {
        assert_eq!(presets(on, &no_locks(), None), None, "{on}");
        assert_eq!(presets(on, &no_locks(), Some(&ALL)), None, "{on}");
    }
    // Seen before the default changed: told once, with the widening.
    let told = presets(
        "[sandbox]\nmode = \"on\"\n",
        &no_locks(),
        Some(&["model-providers", "forge", "toolchains"]),
    )
    .expect("a widening is told");
    assert_eq!(told.added, ["Certificate checks"]);
    assert_eq!(told.removed, Vec::<String>::new());
    assert_eq!(told.now, ALL);
    assert_eq!(told.widens.len(), 1, "{told:?}");
    assert!(told.widens[0].contains("certificate"), "{told:?}");
    // Turned off by the project: told as turned off, on first sight too.
    let off = presets(
        "[sandbox]\nmode = \"on\"\ncertificate-checks = false\n",
        &no_locks(),
        None,
    )
    .expect("told");
    assert_eq!(off.added, Vec::<String>::new());
    assert_eq!(off.removed, ["Certificate checks"]);
    assert_eq!(off.now, ["model-providers", "forge", "toolchains"]);
    // Under policy's lock no chat has them, so they are neither a default seen nor named.
    let locks = super::super::policy::Locks::parse(
        r#"{"sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(presets("[sandbox]\nmode = \"on\"\n", &locks, None), None);
    assert_eq!(
        presets(
            "[sandbox]\nmode = \"on\"\ncertificate-checks = false\n",
            &locks,
            None
        ),
        None
    );
}

#[test]
fn a_preset_turned_back_on_after_it_was_seen_off_is_told_with_the_caches_it_widens() {
    let told = presets(
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"toolchains\"]\n",
        &no_locks(),
        Some(&["model-providers", "certificate-checks"]),
    )
    .expect("a widening is told");
    assert_eq!(told.added, ["Package registries"]);
    assert_eq!(told.removed, Vec::<String>::new());
    assert_eq!(
        told.now,
        ["model-providers", "toolchains", "certificate-checks"]
    );
    assert!(
        told.widens.iter().any(|one| one.contains("package caches")),
        "{told:?}"
    );
}

#[test]
fn a_narrowed_project_is_told_once_so_a_later_widening_is_never_missed() {
    // First sight of a project that turned presets off: told, so what is seen is recorded.
    let told = presets(
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
        &no_locks(),
        None,
    )
    .expect("told");
    assert_eq!(told.added, Vec::<String>::new());
    assert_eq!(told.removed, ["Code hosting", "Package registries"]);
    assert_eq!(told.widens, Vec::<String>::new());
    // Seen as shown: nothing more until it changes.
    assert_eq!(
        presets(
            "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
            &no_locks(),
            Some(&told.now.iter().map(String::as_str).collect::<Vec<_>>()),
        ),
        None
    );
}

#[test]
fn what_was_acknowledged_is_what_was_shown_and_order_is_no_change() {
    assert_eq!(
        presets(
            "[sandbox]\nmode = \"on\"\negress = [\"toolchains\", \"forge\"]\n",
            &no_locks(),
            Some(&["certificate-checks", "forge", "toolchains"]),
        ),
        None
    );
}

#[test]
fn a_preset_policy_turns_off_reaches_no_chat_and_is_not_named() {
    let locks = super::super::policy::Locks::parse(
        r#"{"sandbox": {"presets": ["model-providers"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(
        presets(
            "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"toolchains\"]\n",
            &locks,
            Some(&["model-providers", "certificate-checks"]),
        ),
        None
    );
}

#[test]
fn on_first_sight_a_preset_policy_turns_off_is_not_told_as_turned_off() {
    let locks = super::super::policy::Locks::parse(
        r#"{"sandbox": {"presets": ["model-providers"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    // The project asks for the defaults; policy lets one through. Nothing was turned off.
    assert_eq!(presets("[sandbox]\nmode = \"on\"\n", &locks, None), None);
    // A policy that requires the sandbox: a project with none says nothing either.
    let required = super::super::policy::Locks::parse(
        r#"{"sandbox": {"opt-out": false, "presets": ["model-providers"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(presets("schema = 1\n", &required, None), None);
}

#[test]
fn a_project_whose_chats_are_not_sandboxed_tells_nothing_of_its_presets() {
    assert_eq!(
        presets("[sandbox]\ncertificate-checks = true\n", &no_locks(), None),
        None
    );
    assert_eq!(presets("schema = 1\n", &no_locks(), None), None);
}

/// Over the file: told once, and not again once acknowledged as shown.
#[test]
fn a_teammate_is_told_once_of_a_preset_change() {
    let project = a_project("[sandbox]\nmode = \"on\"\ncertificate-checks = false\n");
    let told = presets_changed(project.path()).expect("told");
    assert_eq!(told.removed, ["Certificate checks"]);
    acknowledge_presets(project.path(), &told.now).expect("kept");
    assert_eq!(presets_changed(project.path()), None, "told once");
    std::fs::write(
        project.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("a teammate's push");
    assert_eq!(
        presets_changed(project.path()).map(|told| told.added),
        Some(vec!["Certificate checks".to_owned()])
    );
}

// -------------------------------------------------------------------------------------
// Seen by you: what this window wrote, never a later read of the disk (#1550)
// -------------------------------------------------------------------------------------

#[test]
fn presets_this_window_wrote_are_seen_and_not_told_back() {
    let written = "[sandbox]\nmode = \"on\"\negress = [\"forge\"]\n";
    let project = a_project(written);
    presets_seen_by_you(project.path(), false, written);
    assert_eq!(presets_changed(project.path()), None);
}

#[test]
fn a_pull_that_lands_after_the_write_is_told_and_not_recorded_as_seen() {
    let written = "[sandbox]\nmode = \"on\"\negress = [\"forge\"]\n";
    // A teammate's change pulled in between the write and the record.
    let project = a_project("[sandbox]\nmode = \"on\"\negress = [\"forge\", \"toolchains\"]\n");
    presets_seen_by_you(project.path(), false, written);
    let told = presets_changed(project.path()).expect("the pulled change is told");
    assert_eq!(told.now, ["forge", "toolchains", "certificate-checks"]);

    let written = "[sandbox]\nmode = \"on\"\nhosts = [\"api.example\"]\n";
    let project = a_project("[sandbox]\nmode = \"on\"\nhosts = [\"api.example\", \"b.example\"]\n");
    hosts_seen_by_you(project.path(), false, written);
    let told = hosts_changed(project.path()).expect("the pulled hosts are told");
    assert_eq!(told.now, ["api.example", "b.example"]);
}

#[test]
fn hosts_this_window_wrote_are_seen_unless_a_change_was_waiting() {
    let written = "[sandbox]\nmode = \"on\"\nhosts = [\"api.example\"]\n";
    let project = a_project(written);
    hosts_seen_by_you(project.path(), true, written);
    assert!(
        hosts_changed(project.path()).is_some(),
        "a waiting change is still told"
    );
    hosts_seen_by_you(project.path(), false, written);
    assert_eq!(hosts_changed(project.path()), None);
}

// -------------------------------------------------------------------------------------
// Your own hosts: the record keeps only what the file still lists (#1405)
// -------------------------------------------------------------------------------------

#[test]
fn a_confirmed_host_the_local_file_no_longer_lists_leaves_the_record_and_one_it_lists_stays() {
    let project = a_project("schema = 1\n");
    for host in ["kept.example.org", "gone.example.org", "[fd00::7]:8443"] {
        confirm_host(project.path(), host).expect("confirmed");
    }

    let dropped = forget_hosts_not_in(
        project.path(),
        Some("[sandbox]\nhosts = [\"KEPT.example.org\", \"[fd00::7]:8443\"]\n"),
    )
    .expect("pruned");

    assert_eq!(dropped, 1);
    assert_eq!(
        confirmed_hosts(project.path()),
        ["kept.example.org", "[fd00::7]:8443"]
    );
}

#[test]
fn no_local_file_lists_no_host_and_the_record_of_them_goes() {
    let project = a_project("schema = 1\n");
    confirm_host(project.path(), "gone.example.org").expect("confirmed");

    assert_eq!(
        forget_hosts_not_in(project.path(), None).expect("pruned"),
        1
    );

    assert!(confirmed_hosts(project.path()).is_empty());
}

#[test]
fn a_local_file_that_is_not_toml_takes_nothing_off_the_record() {
    let project = a_project("schema = 1\n");
    confirm_host(project.path(), "kept.example.org").expect("confirmed");

    let dropped =
        forget_hosts_not_in(project.path(), Some("[sandbox\nhosts = [")).expect("nothing to do");

    assert_eq!(dropped, 0);
    assert_eq!(confirmed_hosts(project.path()), ["kept.example.org"]);
}

#[test]
fn a_record_with_nothing_to_drop_is_not_written() {
    let project = a_project("schema = 1\n");
    std::fs::create_dir_all(path(project.path()).parent().expect("its folder"))
        .expect("its folder");
    std::fs::write(
        path(project.path()),
        "{\"hosts_mine\": [\"kept.example.org\"]}\n",
    )
    .expect("as written by hand");

    let dropped = forget_hosts_not_in(
        project.path(),
        Some("[sandbox]\nhosts = [\"kept.example.org\"]\n"),
    )
    .expect("nothing to do");

    assert_eq!(dropped, 0);
    assert_eq!(
        std::fs::read_to_string(path(project.path())).expect("the record"),
        "{\"hosts_mine\": [\"kept.example.org\"]}\n",
        "the record is as it was"
    );
}

// -------------------------------------------------------------------------------------
// A state file the repository brought (ADR 0093)
// -------------------------------------------------------------------------------------

/// git in `root`, for a fixture.
fn git(root: &Path, args: &[&str]) {
    let mut all = vec![
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ];
    all.extend(args);
    let run = crate::testgit::run(root, &all);
    assert_eq!(run.code, Some(0), "git {args:?}: {}", run.err);
}

/// The file an older build kept in the project (`app/sandbox.json`) is never read (the operator's
/// decision of 2026-10-12): a repository can carry it, and so can a zip, a synced folder, or a
/// copy that brought its own `.git`, untracked. Whatever it says, nothing in it is granted or
/// accepted, and the person is asked again. Needs `git init`, so it first runs on CI.
#[test]
fn an_older_file_in_the_project_grants_nothing_committed_or_not() {
    let project = a_project("schema = 1\n\n[sandbox]\nmode = \"on\"\n");
    let root = project.path().canonicalize().expect("real");
    let legacy = legacy_path(&root);
    std::fs::create_dir_all(legacy.parent().expect("a folder")).expect("made");
    std::fs::write(&legacy, PLANTED).expect("planted");
    // A copy with no repository, then one with its own `.git` and the file untracked, then
    // committed: none is read.
    nothing_taken(&root);
    // Its git directory kept apart, as a clone with `--separate-git-dir` keeps it.
    let kept = tempfile::tempdir().expect("a place for the repository");
    let at = format!("--separate-git-dir={}", kept.path().join("git").display());
    git(&root, &["init", "-q", &at]);
    nothing_taken(&root);
    let rel = legacy
        .strip_prefix(&root)
        .expect("in the project")
        .to_string_lossy()
        .into_owned();
    git(&root, &["add", "-f", "--", &rel]);
    git(&root, &["commit", "-q", "-m", "a state file of its own"]);
    nothing_taken(&root);
}

/// What a repository, or a copy, could plant: a folder, a host of yours, a socket and a service,
/// and the acceptance of every kind of committed grant.
const PLANTED: &str = r#"{
  "writes_mine": ["/opt/anywhere"],
  "hosts_mine": ["a.example"],
  "sockets_mine": ["/var/run/docker.sock"],
  "services_mine": ["com.apple.trustd.agent"],
  "hosts_seen": ["b.example"],
  "presets_seen": ["forge", "certificate-checks"],
  "dispatch_seen": ["steward -> devops"],
  "dispatch_any_seen": ["steward"],
  "dispatch_mine": [{"asking": "steward", "target": "devops"}]
}"#;

/// Nothing of [`PLANTED`] is in force at `root`, and reading wrote nothing.
fn nothing_taken(root: &Path) {
    assert!(granted_writes(root).is_empty());
    assert!(confirmed_hosts(root).is_empty());
    assert!(granted_sockets(root).is_empty());
    assert!(granted_services(root).is_empty());
    assert!(granted_dispatch(root).is_empty());
    assert_eq!(dispatch_seen(root), None);
    assert!(dispatch_any_seen(root).is_empty());
    assert!(!path(root).exists());
}

/// What this machine allows is kept in its config home, never in the project.
#[test]
fn what_this_machine_allows_is_kept_outside_the_project() {
    let project = a_project("schema = 1\n\n[sandbox]\nmode = \"on\"\n");
    let root = project.path().canonicalize().expect("real");
    let folder = root.join("out");
    std::fs::create_dir_all(&folder).expect("made");
    grant_write(&root, &folder).expect("kept");
    assert_eq!(granted_writes(&root), std::slice::from_ref(&folder));
    assert!(!path(&root).starts_with(&root));
    assert!(!legacy_path(&root).exists());
}

/// A folder made again at the same path is another project to this machine: it takes nothing
/// allowed for the one before.
#[test]
fn a_folder_made_again_at_the_same_path_takes_nothing_allowed_before() {
    let base = tempfile::tempdir().expect("a folder");
    let root = base.path().canonicalize().expect("real").join("plane");
    std::fs::create_dir_all(&root).expect("made");
    let folder = root.join("out");
    std::fs::create_dir_all(&folder).expect("made");
    grant_write(&root, &folder).expect("kept");
    assert_eq!(granted_writes(&root), std::slice::from_ref(&folder));
    // Moved aside, and another folder made where it was: the first keeps its inode, so the
    // new one is another.
    std::fs::rename(&root, base.path().join("aside")).expect("moved aside");
    std::fs::create_dir_all(&root).expect("made again");
    assert!(granted_writes(&root).is_empty());
}

/// #1927: the answer to what a project expects is kept by what was shown, the declared path
/// with the name it was shown under. An older build's record, of declared paths alone, reads
/// as answered under the path as written; names answered before are kept for a path still
/// answered, a path no longer answered is forgotten, and the names kept are bounded.
#[test]
fn an_answer_to_what_a_project_expects_is_kept_by_what_was_shown() {
    let project = a_project("schema = 1\n\n[sandbox]\nmode = \"on\"\n");
    let root = project.path().canonicalize().expect("real");
    assert!(expects_answered(&root).is_empty());
    let shown = |written: &str, named: &str| (written.to_owned(), named.to_owned());
    acknowledge_expects(
        &root,
        &[
            shown("/opt/run/dev.sock", "/Users/dev/.tool/dev.sock"),
            shown("com.example.helper", "com.example.helper"),
        ],
    )
    .expect("kept");
    assert_eq!(
        expects_answered(&root),
        [
            Answered {
                written: "/opt/run/dev.sock".to_owned(),
                named: vec!["/Users/dev/.tool/dev.sock".to_owned()],
            },
            Answered {
                written: "com.example.helper".to_owned(),
                named: vec!["com.example.helper".to_owned()],
            },
        ]
    );
    assert_eq!(
        expects_seen(&root),
        ["/opt/run/dev.sock", "com.example.helper"]
    );
    // Answered again under another name: both names stand for that path; the service, not
    // answered this time, is forgotten and asked about again.
    acknowledge_expects(
        &root,
        &[shown("/opt/run/dev.sock", "/Users/dev/.other/x.sock")],
    )
    .expect("kept");
    assert_eq!(
        expects_answered(&root),
        [Answered {
            written: "/opt/run/dev.sock".to_owned(),
            named: vec![
                "/Users/dev/.tool/dev.sock".to_owned(),
                "/Users/dev/.other/x.sock".to_owned()
            ],
        }]
    );
    // Bounded: the oldest names go first.
    for n in 0..(MOST_NAMES_ANSWERED + 3) {
        acknowledge_expects(
            &root,
            &[shown("/opt/run/dev.sock", &format!("/Users/dev/{n}.sock"))],
        )
        .expect("kept");
    }
    let kept = expects_answered(&root);
    assert_eq!(kept[0].named.len(), MOST_NAMES_ANSWERED);
    assert_eq!(
        kept[0].named.last().map(String::as_str),
        Some(format!("/Users/dev/{}.sock", MOST_NAMES_ANSWERED + 2).as_str())
    );
    // An older build's record: each declared path, answered as written.
    write_new(&path(&root), |held| {
        held.expects_answered = None;
        held.expects_seen = Some(vec!["/opt/run/old.sock".to_owned()]);
    })
    .expect("an older record");
    assert_eq!(
        expects_answered(&root),
        [Answered {
            written: "/opt/run/old.sock".to_owned(),
            named: vec!["/opt/run/old.sock".to_owned()],
        }]
    );
}

// -------------------------------------------------------------------------------------
// Always for this persona: a browser (#2241)
// -------------------------------------------------------------------------------------

/// #2241: **Always for this persona** is kept in this machine's own file for the project, beside
/// its other Always answers, with what it reaches; one answer per persona, the last one kept;
/// and Remove takes it and its Granted record away.
#[test]
fn always_for_a_persona_s_browser_is_kept_per_persona_and_removed_whole() {
    let project = a_project("schema = 1\n");
    let root = project.path();
    assert_eq!(browser_always(root, "qa"), None);
    keep_browser_always(root, "qa", false).expect("kept");
    keep_browser_always(root, "research", true).expect("kept");
    assert_eq!(browser_always(root, "qa"), Some(false));
    assert_eq!(browser_always(root, "research"), Some(true));
    // A second answer for the same persona replaces the first.
    keep_browser_always(root, "qa", true).expect("kept");
    assert_eq!(browser_always(root, "qa"), Some(true));
    assert_eq!(browsers_always(root).len(), 2);
    let text = std::fs::read_to_string(path(root)).expect("this machine's file");
    assert!(text.contains("\"browsers_mine\""), "{text}");
    record_made(
        root,
        Made {
            what: BROWSER.to_owned(),
            target: "qa".to_owned(),
            level: "you".to_owned(),
            at: 1,
            chat: None,
        },
    )
    .expect("recorded");
    assert!(revoke_browser_always(root, "qa").expect("removed"));
    assert_eq!(browser_always(root, "qa"), None);
    assert!(made(root).iter().all(|one| one.what != BROWSER));
    assert!(!revoke_browser_always(root, "qa").expect("nothing to remove"));
    // Nothing of it is in the project.
    assert_eq!(charter_toml(root), "schema = 1\n");
}

/// #2241: an administrator's policy forbids Always for this persona by leaving `you` out of
/// `allow-scopes`: one kept before reaches no chat while the policy stands, and is kept.
#[test]
fn a_policy_without_the_you_scope_holds_always_for_a_persona_s_browser() {
    use crate::sandbox::policy::Locks;
    let project = a_project("schema = 1\n");
    let root = project.path();
    keep_browser_always(root, "qa", false).expect("kept");
    assert_eq!(browser_always_here(root, "qa", &Locks::none()), Some(false));
    let no_you = Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["chat", "project"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(browser_always_here(root, "qa", &no_you), None);
    assert_eq!(
        browser_always(root, "qa"),
        Some(false),
        "kept, not in force"
    );
}

/// #2241 security: Always for a persona's browser is kept only in this machine's file for the
/// project, which every sandboxed chat is denied writing; nothing of it is in the project.
#[test]
fn always_for_a_persona_s_browser_is_kept_where_no_sandboxed_chat_writes() {
    let project = a_project("schema = 1\n");
    let root = project.path().canonicalize().expect("real");
    let home = tempfile::tempdir().expect("a config home");
    let home_path = home.path().canonicalize().expect("real");
    TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home_path.clone()));
    keep_browser_always(&root, "qa", true).expect("kept");
    let file = path(&root);
    let kept = std::fs::read_to_string(&file).expect("kept in this machine's file");
    assert!(kept.contains("\"qa\""), "{kept}");
    assert!(file.starts_with(&home_path), "{}", file.display());
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[(
            crate::machine::HOME_VAR,
            home_path.to_str().expect("utf-8"),
        )]),
        home: Some(root.join("home")),
        os: crate::sandbox::Os::MacOs,
    };
    let denied = crate::sandbox::Denied::of(&root, &machine);
    assert!(
        denied
            .paths
            .iter()
            .any(|denial| file.starts_with(&denial.path)),
        "{} is not denied: {:?}",
        file.display(),
        denied.paths
    );
    TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// #2018: a browser site you allowed for every chat's browser here is kept in this machine's own
/// file, apart from the hosts the chats' commands reach, and Remove takes it and its Granted
/// record away.
#[test]
fn a_browser_site_is_kept_apart_from_the_hosts_and_removed_whole() {
    use crate::sandbox::browser_reach::Site;
    let project = a_project("schema = 1\n");
    let root = project.path();
    let site = Site::parse("news.example.org:443").expect("a site");
    grant_browser_site(root, &site).expect("kept");
    grant_browser_site(root, &site).expect("kept once");
    assert_eq!(browser_sites(root), std::slice::from_ref(&site));
    let text = std::fs::read_to_string(path(root)).expect("this machine's file");
    assert!(text.contains("\"browser_sites_mine\""), "{text}");
    assert!(
        crate::sandbox::hosts::personal(root).is_empty(),
        "no host of the chats' commands"
    );
    record_made(
        root,
        Made {
            what: BROWSER_SITE.to_owned(),
            target: site.to_string(),
            level: "you".to_owned(),
            at: 1,
            chat: None,
        },
    )
    .expect("recorded");
    assert!(revoke_browser_site(root, &site).expect("removed"));
    assert!(browser_sites(root).is_empty());
    assert!(made(root).iter().all(|one| one.what != BROWSER_SITE));
    assert!(!revoke_browser_site(root, &site).expect("nothing to remove"));
    assert_eq!(charter_toml(root), "schema = 1\n");
}

/// #2018: a browser site kept here is held, kept and listed but reaching no browser, where an
/// administrator's policy later never allows its host or leaves out the scope it was kept at.
#[test]
fn a_kept_browser_site_policy_now_refuses_reaches_no_browser() {
    use crate::sandbox::browser_reach::Site;
    use crate::sandbox::policy::Locks;
    let project = a_project("schema = 1\n");
    let root = project.path();
    let pasted = Site::parse("bin.paste.example:443").expect("a site");
    let news = Site::parse("news.example.org:443").expect("a site");
    grant_browser_site(root, &pasted).expect("kept");
    grant_browser_site(root, &news).expect("kept");
    let file = Path::new("/etc/purlis/policy.json");
    let never = Locks::parse(r#"{"sandbox": {"never-hosts": ["*.paste.example"]}}"#, file);
    assert!(browser_site_held(&pasted, &never).is_some());
    assert_eq!(
        browser_sites_in_force(root, &never),
        std::slice::from_ref(&news)
    );
    assert_eq!(browser_sites(root).len(), 2, "held, and still kept");
    let scopes = Locks::parse(r#"{"sandbox": {"allow-scopes": ["chat"]}}"#, file);
    assert!(browser_sites_in_force(root, &scopes).is_empty());
    assert_eq!(browser_sites_in_force(root, &Locks::none()), [pasted, news]);
}
