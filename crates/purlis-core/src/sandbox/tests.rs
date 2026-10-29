// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The sandbox policy (ADR 0067): what a plane may say, and nothing it may not. Every expected
//! answer is written out.

use super::*;

fn said(text: &str) -> Said {
    Plane::of(Some(text)).said()
}

fn default_policy() -> Option<Policy> {
    Some(Policy {
        egress: vec![Preset::ModelProviders, Preset::Forge, Preset::Toolchains],
        hosts: vec![],
        certificate_checks: true,
        personas: Default::default(),
    })
}

// -------------------------------------------------------------------------------------
// The schema: `[sandbox]` in `charter.toml`
// -------------------------------------------------------------------------------------

#[test]
fn a_plane_that_says_nothing_runs_its_chats_as_it_always_has() {
    let said = said("schema = 1\n");
    assert_eq!(said.policy, None);
    assert!(said.refused.is_empty(), "{:?}", said.refused);
}

#[test]
fn a_plane_that_turns_the_sandbox_on_gets_the_default_egress() {
    let said = said("[sandbox]\nmode = \"on\"\n");
    assert_eq!(said.policy, default_policy());
    assert!(said.refused.is_empty(), "{:?}", said.refused);
}

#[test]
fn a_plane_s_off_is_the_team_s_and_its_hosts_still_hold_for_a_full_sandbox() {
    // #2071 PR C (the operator's ruling of 2026-10-15): `mode = "off"` turns the sandbox off
    // for the team, reaching each person only on their Accept (`team::team_off`). Its policy
    // is still read, for a person who keeps the full sandbox.
    let said = said("[sandbox]\nmode = \"off\"\n");
    assert_eq!(said.policy, default_policy());
    assert!(said.refused.is_empty(), "{:?}", said.refused);
}

#[test]
fn off_under_a_never_policy_is_refused_naming_the_policy_and_leaves_the_sandbox_on() {
    let said = said("[sandbox]\nmode = \"off\"\npolicy = \"never\"\n");
    assert_eq!(said.policy, default_policy());
    assert_eq!(said.refused, [Refusal::ModeOff]);
    assert_eq!(
        said.refused[0].to_string(),
        "sandbox.mode in charter.toml cannot be \"off\": the project's sandbox.policy is \
         \"never\", so nobody turns the sandbox off; so the sandbox is on"
    );
}

#[test]
fn a_policy_that_is_not_never_or_local_is_refused_and_read_as_never() {
    let said = said("[sandbox]\nmode = \"on\"\npolicy = \"team-off\"\n");
    assert_eq!(said.refused, [Refusal::PolicyUnknown]);
    assert_eq!(
        said.refused[0].to_string(),
        "sandbox.policy in charter.toml is not \"never\" or \"local\", so it is read as \
         \"never\": nobody turns the sandbox down until it says one"
    );
}

#[test]
fn a_mistyped_mode_turns_the_sandbox_on_rather_than_leaving_it_off() {
    for mode in ["\"onn\"", "\"On\"", "true", "0"] {
        let said = said(&format!("[sandbox]\nmode = {mode}\n"));
        assert_eq!(said.policy, default_policy(), "{mode}");
        assert_eq!(said.refused, [Refusal::ModeUnknown], "{mode}");
    }
}

#[test]
fn a_sandbox_that_is_not_a_table_turns_it_on() {
    let said = said("sandbox = \"off\"\n");
    assert_eq!(said.policy, default_policy());
    assert_eq!(said.refused, [Refusal::NotATable]);
}

#[test]
fn a_plane_names_its_egress_by_preset_and_an_unknown_preset_is_refused() {
    let said = said("[sandbox]\nmode = \"on\"\negress = [\"forge\", \"everywhere\"]\n");
    assert_eq!(
        said.policy,
        Some(Policy {
            egress: vec![Preset::Forge],
            hosts: vec![],
            certificate_checks: true,
            personas: Default::default(),
        })
    );
    assert_eq!(
        said.refused
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "sandbox.egress in charter.toml names \"everywhere\", which is not a preset — one of: \
          model-providers, forge, toolchains"
        ]
    );
}

#[test]
fn an_empty_egress_list_is_the_strictest_answer_and_is_kept() {
    let said = said("[sandbox]\nmode = \"on\"\negress = []\n");
    assert_eq!(
        said.policy,
        Some(Policy {
            egress: vec![],
            hosts: vec![],
            certificate_checks: true,
            personas: Default::default(),
        })
    );
}

#[test]
fn a_key_the_schema_does_not_have_is_refused() {
    let said = said("[sandbox]\nmode = \"on\"\nwritable = [\"/\"]\n");
    assert_eq!(said.refused, [Refusal::UnknownKey("writable".to_owned())]);
    assert!(said.policy.is_some());
}

#[test]
fn this_machine_s_file_holds_its_own_hosts_and_never_the_mode() {
    let off = "[sandbox]\nmode = \"off\"\n";
    assert_eq!(refusals(off, "charter.toml").len(), 0);
    assert_eq!(
        refusals(off, "charter.local.toml"),
        [
            "sandbox.mode in charter.local.toml is not read — this machine's [sandbox] holds hosts \
             and your own level (level, personas), and nothing else. The project's mode and \
             presets are the project's, in charter.toml."
        ]
    );
    let mine = "[sandbox]\nhosts = [\"10.100.39.145:6443\"]\n";
    assert_eq!(refusals(mine, "charter.local.toml"), Vec::<String>::new());
    let bad = "[sandbox]\nhosts = [\"127.0.0.1\"]\n";
    let said = refusals(bad, "charter.local.toml");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(
            "sandbox.hosts in charter.local.toml names \"127.0.0.1\", which no chat is let reach: \
             127.0.0.1 is this machine."
        ),
        "{said:?}"
    );
    // Any other file says nothing about the sandbox.
    assert_eq!(refusals(off, "workspace.json"), Vec::<String>::new());
}

#[test]
fn a_project_names_its_own_hosts_and_a_host_that_is_not_one_is_refused_and_dropped() {
    let said = said(
        "[sandbox]\nmode = \"on\"\nhosts = [\"10.100.39.145:6443\", \"*.internal.example\", \
         \"https://x.example/\", 7]\n",
    );
    assert_eq!(
        said.policy.expect("on").hosts,
        [
            hosts::Host::parse("10.100.39.145:6443").unwrap(),
            hosts::Host::parse("*.internal.example").unwrap(),
        ]
    );
    let refused: Vec<String> = said.refused.iter().map(ToString::to_string).collect();
    assert_eq!(
        refused,
        [
            "sandbox.hosts in charter.toml names \"https://x.example/\", which no chat is let \
             reach: That is a URL. Type its host alone, such as x.example, without the scheme or \
             a path.",
            "sandbox.hosts in charter.toml names 7, which no chat is let reach: A host is written \
             as text in quotes.",
        ]
    );
}

#[test]
fn hosts_that_are_not_a_list_grant_nothing() {
    let said = said("[sandbox]\nmode = \"on\"\nhosts = \"10.0.0.5\"\n");
    assert_eq!(said.refused, [Refusal::HostsNotAList]);
    assert_eq!(said.policy.expect("on").hosts, []);
}

// -------------------------------------------------------------------------------------
// The denial classes (ADR 0067 §5): each one on its own, and none a plane can remove
// -------------------------------------------------------------------------------------

/// A plane on a machine whose home is `/home/op`, with `vaults` as its committed registry.
fn denied_with(vaults: Option<&str>, os: Os) -> (tempfile::TempDir, Denied) {
    let plane = tempfile::tempdir().expect("a plane");
    if let Some(vaults) = vaults {
        std::fs::write(plane.path().join("vaults.json"), vaults).expect("the registry");
    }
    let denied = Denied::of(plane.path(), &machine(os));
    (plane, denied)
}

fn paths(denied: &Denied, class: Class, access: Access) -> Vec<std::path::PathBuf> {
    denied
        .paths
        .iter()
        .filter(|it| it.class == class && it.access == access)
        .map(|it| it.path.clone())
        .collect()
}

#[test]
fn the_five_classes_are_the_adrs_five() {
    assert_eq!(
        Class::ALL.map(Class::word),
        [
            "vaults",
            "integrity",
            "human-powers",
            "runner-internals",
            "later-code"
        ]
    );
}

#[test]
fn a_chat_never_reads_or_writes_a_vaults_storage() {
    let (plane, denied) = denied_with(
        Some(
            r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "secrets/dev.json"}}}}"#,
        ),
        Os::Linux,
    );
    let root = plane.path();
    assert_eq!(
        paths(&denied, Class::Vaults, Access::ReadWrite),
        [
            root.join(".charter/vaults"),
            root.join(".charter/fingerprint.key"),
            // Every spelling of the state folder, there or not.
            root.join(".purlis/vaults"),
            root.join(".purlis/fingerprint.key"),
            root.join("secrets/dev.json"),
            std::path::PathBuf::from("/home/op/.config/op"),
            std::path::PathBuf::from("/home/op/.op"),
            std::path::PathBuf::from("/home/op/.vault-token"),
        ]
    );
}

#[test]
fn a_sandboxed_chat_may_read_and_run_charters_git_hooks_and_never_rewrite_them() {
    // SQ-16, ADR 0074: git treats a hook it cannot read or run as absent and commits unscanned,
    // so nothing a chat is denied may cover the directory. The directory is the app's data
    // directory on macOS (Tauri's, under the bundle's identifier) — outside the chat's own
    // directory, which is all Claude Code's sandbox lets a command write — so a chat reads and
    // runs it and cannot rewrite it.
    let (_plane, denied) = denied_with(None, Os::MacOs);
    let hooks =
        std::path::PathBuf::from("/home/op/Library/Application Support/dev.charter.app/git-hooks");
    for denial in &denied.paths {
        assert!(
            denial.access == Access::Write || !hooks.starts_with(&denial.path),
            "{} is denied to a chat's reads and holds purlis's git hooks",
            denial.path.display()
        );
    }
    let settings = claude::settings(&compiled(denied, Os::MacOs)).expect("compiles");
    let deny_read = &settings.sandbox["filesystem"]["denyRead"];
    for denied in deny_read.as_array().expect("a list") {
        let denied = std::path::Path::new(denied.as_str().expect("a path"));
        assert!(!hooks.starts_with(denied), "{deny_read}");
    }
}

#[test]
fn a_chat_may_read_the_registry_that_names_vaults_but_never_rewrite_it() {
    let (plane, denied) = denied_with(None, Os::Linux);
    let root = plane.path();
    assert_eq!(
        paths(&denied, Class::Vaults, Access::Write),
        [root.join("vaults.json"), root.join(".charter/vaults.json")]
    );
}

#[test]
fn a_keyring_vault_is_denied_as_the_operating_systems_credential_service() {
    let (_plane, denied) = denied_with(
        Some(r#"{"vaults": {"dev": {"provider": "keyring"}}}"#),
        Os::MacOs,
    );
    assert_eq!(denied.services, [Service::CredentialStore]);
}

#[test]
fn a_plane_with_no_keyring_vault_asks_nothing_of_the_credential_service() {
    let (_plane, denied) = denied_with(
        Some(r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "d.json"}}}}"#),
        Os::MacOs,
    );
    assert_eq!(denied.services, []);
}

#[test]
fn a_registry_charter_cannot_read_is_taken_to_hold_a_keyring_vault() {
    // Fail closed: a corrupt registry could be hiding the default provider.
    let (_plane, denied) = denied_with(Some("not json"), Os::MacOs);
    assert_eq!(denied.services, [Service::CredentialStore]);
}

#[test]
fn a_chat_never_writes_charters_integrity_state() {
    // Under both names of the state folder (RN-2a): the purlis one is read when it is there, so
    // a chat that could make `.purlis/app` would be writing the records charter reads next.
    let (plane, denied) = denied_with(None, Os::Linux);
    let mut held = vec![
        plane.path().join(".purlis/app"),
        // Every chat's theme drafts (#2332): only the app writes one, for the chat that asks.
        plane.path().join(".purlis/theme-drafts"),
        // A skill delete's kept skills and running deletes' notes (SK-7, SK-35).
        plane.path().join(".purlis/skills-deleted"),
        plane.path().join(".purlis/skills-deleting"),
        // Every report a chat is handed: the app's alone to write.
        plane.path().join(".purlis/handbacks"),
        plane.path().join(".purlis/harness-profiles-launched.json"),
        plane
            .path()
            .join(".purlis/harness-declarations-approved.json"),
        plane.path().join(".purlis/mcp-approved.json"),
        // Every chat's settings file (SK-9).
        plane.path().join(".purlis/chat-settings"),
        plane.path().join(".charter/app"),
        plane.path().join(".charter/theme-drafts"),
        // A skill delete's kept skills and running deletes' notes (SK-7, SK-35).
        plane.path().join(".charter/skills-deleted"),
        plane.path().join(".charter/skills-deleting"),
        plane.path().join(".charter/handbacks"),
        plane.path().join(".charter/harness-profiles-launched.json"),
        plane
            .path()
            .join(".charter/harness-declarations-approved.json"),
        plane.path().join(".charter/mcp-approved.json"),
        plane.path().join(".charter/chat-settings"),
        // Neither folder is there yet, so neither is the chat's to make (D-RN2a-7).
        plane.path().join(".purlis"),
        plane.path().join(".charter"),
        // The folder chats' servers listen in (#2019).
        door::root_in(None),
    ];
    // The app's own state of every project (#3718), and the folders that hold it.
    for data in Homes::data_homes(&machine(Os::Linux)) {
        held.extend([
            data.join(crate::appstate::DIR),
            data.join(crate::appstate::DIR),
            data,
        ]);
    }
    // The Skill studio's records (SK-9, SK-16, SK-58, SK-59), under both names.
    for state in [".purlis", ".charter"] {
        held.extend(SKILL_STUDIO_RECORDS.map(|record| plane.path().join(state).join(record)));
    }
    assert_eq!(paths(&denied, Class::Integrity, Access::Write), held);
}

/// #1458: the persona MCP approvals are read where `$PURLIS_HOME` puts the state folder, so
/// they are held there too.
#[test]
fn the_persona_mcp_approvals_are_held_where_purlis_home_puts_them() {
    let plane = tempfile::tempdir().expect("a plane");
    let machine = Machine {
        env: crate::secrets::Env::of(&[("PURLIS_HOME", "/srv/purlis-state")]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: Os::Linux,
    };
    let denied = Denied::of(plane.path(), &machine);
    assert!(
        paths(&denied, Class::Integrity, Access::Write).contains(&std::path::PathBuf::from(
            "/srv/purlis-state/mcp-approved.json"
        )),
        "{:?}",
        denied.paths
    );
    // And the theme drafts the app writes there (#2332).
    assert!(
        paths(&denied, Class::Integrity, Access::Write)
            .contains(&std::path::PathBuf::from("/srv/purlis-state/theme-drafts")),
        "{:?}",
        denied.paths
    );
    // And every chat's settings file a start writes there (SK-9).
    assert!(
        paths(&denied, Class::Integrity, Access::Write)
            .contains(&std::path::PathBuf::from("/srv/purlis-state/chat-settings")),
        "{:?}",
        denied.paths
    );
    // And what skill deletes keep there (SK-7, SK-35), and the Skill studio's records.
    for kept in ["skills-deleted", "skills-deleting"]
        .into_iter()
        .chain(SKILL_STUDIO_RECORDS)
    {
        assert!(
            paths(&denied, Class::Integrity, Access::Write)
                .contains(&std::path::Path::new("/srv/purlis-state").join(kept)),
            "{kept}: {:?}",
            denied.paths
        );
    }
}

#[test]
fn a_chat_cannot_plant_a_purlis_state_folder_beside_charters() {
    // D-RN2a-7: a `.purlis/` made beside `.charter/` must not be the chat's to make. The one
    // that is there is not denied whole: the rows for what is inside it hold.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::create_dir(plane.path().join(".charter")).unwrap();
    let denied = Denied::of(plane.path(), &machine(Os::Linux));
    let write = paths(&denied, Class::Integrity, Access::Write);
    assert!(write.contains(&plane.path().join(".purlis")), "{write:?}");
    // The one there is held as an entry alone, never with what is below it.
    let charter = plane.path().join(".charter");
    assert!(
        denied
            .paths
            .iter()
            .filter(|it| it.path == charter)
            .all(|it| it.reach == Reach::Itself),
        "{:?}",
        denied.paths
    );
}

#[test]
fn a_state_folder_name_held_by_a_file_or_a_link_is_still_not_the_chats_to_make() {
    // "Is not a directory", not "is absent": a committed file or a link named `.purlis` would
    // otherwise be one `rm` away from a folder the chat fills.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::create_dir(plane.path().join(".charter")).unwrap();
    std::fs::write(plane.path().join(".purlis"), "").unwrap();
    let denied = Denied::of(plane.path(), &machine(Os::Linux));
    let write = paths(&denied, Class::Integrity, Access::Write);
    assert!(write.contains(&plane.path().join(".purlis")), "{write:?}");

    std::fs::remove_file(plane.path().join(".purlis")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(plane.path().join(".charter"), plane.path().join(".purlis"))
        .unwrap();
    let denied = Denied::of(plane.path(), &machine(Os::Linux));
    let write = paths(&denied, Class::Integrity, Access::Write);
    assert!(write.contains(&plane.path().join(".purlis")), "{write:?}");
}

/// Both names of the committed manifest and of the machine's local one.
const MANIFEST_NAMES: [&str; 4] = [
    "charter.toml",
    "purlis.toml",
    "charter.local.toml",
    "purlis.local.toml",
];

#[test]
fn no_manifest_is_held_by_name_at_any_depth() {
    // ADR 0067 §5 as amended on 2026-10-06: held only where it could change a chat's
    // sandbox, so a clone's fixtures and a temp folder may carry one.
    for name in MANIFEST_NAMES {
        assert!(
            !PLANTED.iter().any(|planted| planted.path == name),
            "{name}"
        );
    }
}

#[test]
fn the_project_roots_manifests_are_denied_under_every_name_whatever_the_chats_folder() {
    let plane = tempfile::tempdir().expect("a plane");
    let denied = Denied::of(plane.path(), &machine(Os::Linux));
    for name in MANIFEST_NAMES {
        let want = plane.path().join(name);
        assert!(
            denied.paths.iter().any(|it| it.path == want
                && it.class == Class::LaterCode
                && it.access == Access::Write),
            "{name}: {:?}",
            denied.paths
        );
    }
}

/// The second acceptance line of #667, as V22a words it: a chat cannot write another chat's
/// spool. A sandboxed chat neither reads nor writes any chat's, its own included, nor the keys
/// that check them: the hooks of the harnesses charter sandboxes run outside the sandbox their
/// tools run in (ADR 0068 §6, V63).
///
/// #980: asserted in what every compiler starts from **and** in each harness's compiled form,
/// on both systems: Claude Code's own sandbox and file-tool denials, and purlis's wrap that
/// holds a Codex or opencode chat with its Seatbelt profile. On Linux neither wrap compiles,
/// so no such chat is started (fail closed) and there is no form of theirs to hold a spool.
/// The hook channel's per-user fallback directory is #951's: `claude_tests` and `seatbelt_tests`
/// hold it for each harness.
#[cfg(unix)]
#[test]
fn a_chat_never_reads_or_writes_any_chats_hook_spool_or_its_keys() {
    for os in [Os::MacOs, Os::Linux] {
        let (plane, denied) = denied_with(None, os);
        let files: Vec<std::path::PathBuf> = [".charter", ".purlis"]
            .iter()
            .flat_map(|state| {
                let spool = crate::hookwire::spool::dir_for(
                    &plane.path().join(state).join("app/hooks.sock"),
                );
                [
                    spool.join("6").join("0123456789abcdef.1.json"),
                    spool.join("6.jsonl"),
                    spool.join(crate::hookwire::spool::KEYS),
                ]
            })
            .collect();
        let under = |held: &[std::path::PathBuf], file: &std::path::Path| {
            held.iter().any(|folder| file.starts_with(folder))
        };

        // What every compiler starts from.
        let held = paths(&denied, Class::Integrity, Access::ReadWrite);
        for file in &files {
            assert!(
                under(&held, file),
                "{os:?}: {} is not under {held:?}",
                file.display()
            );
        }

        // Claude Code: its own sandbox's read and write denials, and its file tools'. Its
        // hooks run outside that sandbox.
        let settings = claude::settings(&compiled(denied.clone(), os)).expect("compiles");
        let listed = |key: &str| -> Vec<std::path::PathBuf> {
            settings.sandbox["filesystem"][key]
                .as_array()
                .expect("a list")
                .iter()
                .filter_map(|path| path.as_str().map(std::path::PathBuf::from))
                .collect()
        };
        let tools_read: Vec<std::path::PathBuf> = settings
            .deny
            .iter()
            .filter_map(|rule| rule.strip_prefix("Read(/")?.strip_suffix("/**)"))
            .map(std::path::PathBuf::from)
            .collect();
        for file in &files {
            // As the kernel names it, which is what the compiler hands the harness.
            let named = super::real(file);
            assert!(under(&listed("denyRead"), &named), "{os:?}: {named:?}");
            assert!(under(&listed("denyWrite"), &named), "{os:?}: {named:?}");
            assert!(
                under(&tools_read, &named),
                "{os:?}: {named:?}: {:?}",
                settings.deny
            );
        }

        // Codex and opencode run inside purlis's own wrap, on macOS and on Linux (#1040).
        let codex = codex::wrap(&compiled(denied.clone(), os));
        let opencode = applied_for(
            Harness::Opencode,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(os),
        );
        let read_write = |denied: &[Denial]| -> Vec<std::path::PathBuf> {
            denied
                .iter()
                .filter(|it| it.access == Access::ReadWrite)
                .map(|it| it.path.clone())
                .collect()
        };
        let codex = codex.expect("compiles");
        let applied = opencode.expect("compiles");
        let Form::Opencode(wrap) = applied.form() else {
            panic!("compiled for opencode");
        };
        let held_codex = read_write(&codex.denied);
        for file in &files {
            assert!(under(&held_codex, file), "codex: {}", file.display());
            assert!(
                under(&read_write(&wrap.denied), file),
                "opencode: {}",
                file.display()
            );
        }

        // And the Seatbelt profile the wrap runs them under says so, under both firmlink
        // names of the folder that holds each spool.
        let profile = seatbelt::profile(
            &codex.denied,
            &seatbelt::Own::default(),
            &plane.path().join("workspaces/alpha"),
            std::path::Path::new("/private/tmp/chat"),
            &[4040],
            None,
        )
        .expect("a profile");
        for file in &files {
            let folder = held_codex
                .iter()
                .find(|folder| file.starts_with(folder))
                .expect("held above");
            for rule in held_rules(folder, Access::ReadWrite, os == Os::MacOs) {
                assert!(profile.contains(&rule), "{rule} missing:\n{profile}");
            }
        }
    }
}

/// #1452, D-1452-11: the dispatch records hold briefs and reports, and a chat running as one
/// persona reads none written for another. Denied for reading and writing, under both names of
/// the state folder, in what every harness is compiled.
#[test]
fn every_harness_denies_a_chat_reading_or_writing_the_dispatch_records() {
    let (plane, denied) = denied_with(None, Os::MacOs);
    let stores: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .map(|state| plane.path().join(state).join("app").join("dispatches"))
        .collect();
    let held = paths(&denied, Class::Integrity, Access::ReadWrite);
    for store in &stores {
        assert!(held.contains(store), "{} not in {held:?}", store.display());
    }

    // Claude Code: its own sandbox's read and write denials, and its file tools'.
    let settings = claude::settings(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    let listed = |key: &str| -> Vec<String> {
        settings.sandbox["filesystem"][key]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|path| path.as_str().map(str::to_owned))
            .collect()
    };
    for store in &stores {
        // As the kernel names it, which is what the compiler hands the harness.
        let named = super::real(store).display().to_string();
        assert!(listed("denyRead").contains(&named), "{named}");
        assert!(listed("denyWrite").contains(&named), "{named}");
        assert!(
            settings.deny.contains(&format!("Read(/{named}/**)")),
            "{named}: {:?}",
            settings.deny
        );
    }

    // Codex and opencode: purlis's own wrap, whose profile denies both.
    let codex = codex::wrap(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    for store in &stores {
        assert!(
            codex
                .denied
                .iter()
                .any(|it| it.path == *store && it.access == Access::ReadWrite),
            "{}: {:?}",
            store.display(),
            codex.denied
        );
    }
    let cwd = plane.path().join("workspaces/alpha");
    let profile = seatbelt::profile(
        &denied.paths,
        &seatbelt::Own::default(),
        &cwd,
        std::path::Path::new("/private/tmp/chat"),
        &[4040],
        None,
    )
    .expect("a profile");
    for store in &stores {
        for rule in held_rules(store, Access::ReadWrite, true) {
            assert!(profile.contains(&rule), "{rule} missing:\n{profile}");
        }
    }
}

/// #1457, D-1452-12: what a chat's harness said its session cost is the figure purlis shows a
/// dispatch's cost and a session's tokens from, so no sandboxed chat may write it, its own or
/// another's. It is
/// kept in the app's own folder of the state ([`crate::usage::spend_dir`]), which every harness
/// is compiled to deny a chat writing, under both names of the state folder, on both systems.
#[test]
fn every_harness_denies_a_chat_writing_what_its_harness_said_it_cost() {
    for os in [Os::MacOs, Os::Linux] {
        let (plane, denied) = denied_with(None, os);
        let stores: Vec<std::path::PathBuf> = [".charter", ".purlis"]
            .iter()
            .map(|state| {
                plane
                    .path()
                    .join(state)
                    .join("app")
                    .join(crate::usage::SPEND_DIR_NAME)
            })
            .collect();
        // The folder the figures are written to is one of them.
        assert!(stores.contains(&crate::usage::spend_dir(plane.path())));
        let writes = |it: &Denial| matches!(it.access, Access::Write | Access::ReadWrite);
        let under = |held: &[std::path::PathBuf], file: &std::path::Path| {
            held.iter().any(|folder| file.starts_with(folder))
        };

        // What every compiler starts from.
        let held: Vec<std::path::PathBuf> = denied
            .paths
            .iter()
            .filter(|it| it.class == Class::Integrity && writes(it))
            .map(|it| it.path.clone())
            .collect();
        for store in &stores {
            assert!(under(&held, store), "{os:?}: {}", store.display());
        }

        // Claude Code: its own sandbox's write denials. Its status line, the one writer, runs
        // outside that sandbox.
        let settings = claude::settings(&compiled(denied.clone(), os)).expect("compiles");
        let denied_writes: Vec<std::path::PathBuf> = settings.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|path| path.as_str().map(std::path::PathBuf::from))
            .collect();
        for store in &stores {
            let named =
                super::real(store.parent().expect("a folder")).join(crate::usage::SPEND_DIR_NAME);
            assert!(under(&denied_writes, &named), "{os:?}: {named:?}");
        }

        // Codex and opencode: purlis's own wrap, which holds their whole harness, so none of
        // theirs writes a figure (and neither reports one today). None compiles on Linux.
        let Ok(codex) = codex::wrap(&compiled(denied.clone(), os)) else {
            assert_eq!(os, Os::Linux, "codex compiles on macOS");
            continue;
        };
        let held: Vec<std::path::PathBuf> = codex
            .denied
            .iter()
            .filter(|it| writes(it))
            .map(|it| it.path.clone())
            .collect();
        for store in &stores {
            assert!(under(&held, store), "codex: {}", store.display());
        }
    }
}

/// #2017: a chat's browser timeline holds the screenshots its agent was given, which are not
/// for a chat running as another persona: no chat reads or writes any, under either name of
/// the state folder, there or not.
#[test]
fn every_harness_denies_a_chat_every_chats_browser_timeline() {
    for os in [Os::MacOs, Os::Linux] {
        let (plane, denied) = denied_with(None, os);
        let held = paths(&denied, Class::Integrity, Access::ReadWrite);
        for state in [".charter", ".purlis"] {
            let shot = plane
                .path()
                .join(state)
                .join(crate::browsertimeline::DIR)
                .join("01JB0000000000000000000000")
                .join("0.png");
            assert!(
                held.iter().any(|folder| shot.starts_with(folder)),
                "{os:?}: {}",
                shot.display()
            );
        }
    }
}

/// #1507: the record of what was refused while nobody was there is in the dispatch store, and
/// is what a standing grant is offered from. **Its own path** is denied for reading and
/// writing in what each harness is compiled, on both systems, under both names of the state
/// folder, **in a project where neither the file nor its folder is there yet**: the denial is
/// compiled from the project's root, not from what a chat finds on the disk.
#[test]
fn every_harness_denies_a_chat_the_record_of_what_was_refused_while_nobody_was_there() {
    for os in [Os::MacOs, Os::Linux] {
        let (plane, denied) = denied_with(None, os);
        let files: Vec<std::path::PathBuf> = [".charter", ".purlis"]
            .iter()
            .map(|state| {
                plane
                    .path()
                    .join(state)
                    .join("app")
                    .join(crate::dispatchrecord::DIR_NAME)
                    .join(crate::dispatchaway::FILE_NAME)
            })
            .collect();
        for file in &files {
            assert!(!file.parent().expect("a folder").exists(), "not made yet");
        }
        // The file the app writes is one of them.
        assert!(files.contains(&crate::dispatchaway::path(plane.path())));
        let under = |held: &[std::path::PathBuf], file: &std::path::Path| {
            held.iter().any(|folder| file.starts_with(folder))
        };

        // What every compiler starts from.
        let held = paths(&denied, Class::Integrity, Access::ReadWrite);
        for file in &files {
            assert!(under(&held, file), "{os:?}: {}", file.display());
        }

        // Claude Code: its own sandbox's read and write denials, and its file tools'.
        let settings = claude::settings(&compiled(denied.clone(), os)).expect("compiles");
        let listed = |key: &str| -> Vec<std::path::PathBuf> {
            settings.sandbox["filesystem"][key]
                .as_array()
                .expect("a list")
                .iter()
                .filter_map(|path| path.as_str().map(std::path::PathBuf::from))
                .collect()
        };
        for file in &files {
            // As the kernel names the folder, which is what the compiler hands the harness.
            let folder = super::real(file.parent().expect("a folder"));
            let named = folder.join(crate::dispatchaway::FILE_NAME);
            assert!(under(&listed("denyRead"), &named), "{os:?}: {named:?}");
            assert!(under(&listed("denyWrite"), &named), "{os:?}: {named:?}");
            assert!(
                settings
                    .deny
                    .contains(&format!("Read(/{}/**)", folder.display())),
                "{os:?}: {named:?}: {:?}",
                settings.deny
            );
        }

        // Codex and opencode run inside purlis's own wrap, on macOS and on Linux (#1040).
        let codex = codex::wrap(&compiled(denied.clone(), os));
        let opencode = applied_for(
            Harness::Opencode,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(os),
        );
        let read_write = |denied: &[Denial]| -> Vec<std::path::PathBuf> {
            denied
                .iter()
                .filter(|it| it.access == Access::ReadWrite)
                .map(|it| it.path.clone())
                .collect()
        };
        let codex = read_write(&codex.expect("compiles").denied);
        let applied = opencode.expect("compiles");
        let Form::Opencode(wrap) = applied.form() else {
            panic!("compiled for opencode");
        };
        let opencode = read_write(&wrap.denied);
        for file in &files {
            assert!(under(&codex, file), "codex: {}", file.display());
            assert!(under(&opencode, file), "opencode: {}", file.display());
        }
    }
}

/// The same, in a wrapped opencode chat's own compiled form.
#[test]
fn an_opencode_chat_is_handed_the_dispatch_records_denied_for_reading() {
    let plane = tempfile::tempdir().expect("a plane");
    let applied = applied_for(
        Harness::Opencode,
        &policy_of(&[], false),
        &Plane::of(None),
        plane.path(),
        &machine(Os::MacOs),
    )
    .expect("compiles");
    let Form::Opencode(wrap) = applied.form() else {
        panic!("compiled for opencode");
    };
    for state in [".charter", ".purlis"] {
        let store = plane.path().join(state).join("app").join("dispatches");
        assert!(
            wrap.denied
                .iter()
                .any(|it| it.path == store && it.access == Access::ReadWrite),
            "{}: {:?}",
            store.display(),
            wrap.denied
        );
    }
}

/// #1458: the person's approvals of a profile's command and of a project's harness declaration
/// are what lets purlis start that program, so no sandboxed chat writes them, nor the folder
/// the declarations are read from, even standing at the project root with a state folder of
/// its own there. Denied for writing (both are read where a chat may read), in what every
/// harness is compiled, under both names of the state folder.
#[test]
fn every_harness_denies_a_chat_writing_the_persons_harness_approvals_or_the_declarations() {
    let plane = tempfile::tempdir().expect("a plane");
    // A state folder that is there, so the folder itself is not what holds the records.
    std::fs::create_dir(plane.path().join(".purlis")).expect("a state folder");
    let records: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .flat_map(|state| {
            [
                crate::profiletrust::RECORD,
                crate::harness_declaration::APPROVED,
                crate::personaverbs::mcp::APPROVED_FILE,
                // And every chat's settings file, which its harness starts on (SK-9).
                crate::hoststart::SETTINGS_DIR,
            ]
            .map(|record| plane.path().join(state).join(record))
        })
        .collect();
    let declarations = plane.path().join(crate::harness_declaration::DIR);

    // The classes: the records are purlis's own state; the declarations are what a program run
    // later, outside any sandbox, is started from.
    let denied = Denied::of(plane.path(), &machine(Os::MacOs));
    let integrity = paths(&denied, Class::Integrity, Access::Write);
    for record in &records {
        assert!(
            integrity.contains(record),
            "{} not in {integrity:?}",
            record.display()
        );
    }
    assert!(
        paths(&denied, Class::LaterCode, Access::Write).contains(&declarations),
        "{} not held: {:?}",
        declarations.display(),
        denied.paths
    );
    // Read, never written: the start that checks them may run where the chat's reads are held.
    for held in records.iter().chain([&declarations]) {
        assert!(
            !denied
                .paths
                .iter()
                .any(|it| it.access == Access::ReadWrite && held.starts_with(&it.path)),
            "{} is denied to reads",
            held.display()
        );
    }

    let held: Vec<&std::path::PathBuf> = records.iter().chain([&declarations]).collect();
    let mut compiled_harnesses = 0;
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        compiled_harnesses += 1;
        let applied = applied_for(
            harness,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(Os::MacOs),
        )
        .expect("compiles");
        match applied.form() {
            // Claude Code: its own sandbox's write denials, and its file tools'.
            Form::ClaudeCode(settings) => {
                let deny_write: Vec<&str> = settings.sandbox["filesystem"]["denyWrite"]
                    .as_array()
                    .expect("a list")
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                for path in &held {
                    let named = super::real(path).display().to_string();
                    assert!(deny_write.contains(&named.as_str()), "{named}");
                    assert!(
                        settings.deny.contains(&format!("Edit(/{named})")),
                        "{named}: {:?}",
                        settings.deny
                    );
                }
            }
            // Codex and opencode: purlis's own wrap, whose profile a chat at the root runs in.
            Form::Codex(_) | Form::Opencode(_) => {
                let wrapped = applied.form().denied().expect("a wrap");
                for path in &held {
                    assert!(
                        wrapped
                            .iter()
                            .any(|it| it.path == **path && it.access == Access::Write),
                        "{harness:?}: {} not denied",
                        path.display()
                    );
                }
                let profile = seatbelt::profile(
                    wrapped,
                    &seatbelt::Own::default(),
                    plane.path(),
                    std::path::Path::new("/private/tmp/chat"),
                    &[4040],
                    None,
                )
                .expect("a profile");
                for path in &held {
                    // What is in the state folder is held in every case on macOS.
                    let in_state = path.starts_with(plane.path().join(".charter"))
                        || path.starts_with(plane.path().join(".purlis"));
                    for rule in held_rules(path, Access::Write, in_state) {
                        assert!(profile.contains(&rule), "{harness:?}: {rule} missing");
                    }
                }
            }
        }
    }
    assert_eq!(compiled_harnesses, 3, "Claude Code, Codex and opencode");
}

/// The Skill studio's records in the state folder, as written out: each is the app's or the
/// session host's alone to write (SK-9, SK-16, SK-58, SK-59).
const SKILL_STUDIO_RECORDS: [&str; 9] = [
    "skill-levels",
    "skill-drafts",
    "skill-tries",
    "skill-grades",
    "skill-grade-chats",
    "grades-under-way",
    "grades-under-way.lock",
    "skill-grade-after-edit.json",
    "skill-grade-after-edit.lock",
];

/// SK-9, SK-16, SK-58, SK-59: **the Skill studio's records in the state folder are purlis's
/// alone to write**. The levels records say which workspace and persona folders a running chat
/// could have made, and hold their skills until the person accepts them; the drafts, the tries
/// and grades' arms, the grades, the grades under way, the grades' chats and the opt-in to grade
/// after each edit are each written by the app and read back as its own. Each is held with what
/// is below it, so neither a record nor the store as an entry is written, removed or moved, in
/// what every harness is compiled, under both names of the state folder. Read, never written.
#[test]
fn every_harness_denies_a_chat_writing_the_skill_studio_s_records() {
    let plane = tempfile::tempdir().expect("a plane");
    // A state folder that is there, so the folder itself is not what holds the records.
    std::fs::create_dir(plane.path().join(".purlis")).expect("a state folder");
    let records: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .flat_map(|state| SKILL_STUDIO_RECORDS.map(|record| plane.path().join(state).join(record)))
        .collect();

    let denied = Denied::of(plane.path(), &machine(Os::MacOs));
    for record in &records {
        assert!(
            denied.paths.iter().any(|it| it.path == *record
                && it.class == Class::Integrity
                && it.access == Access::Write
                && it.reach == Reach::AndBelow),
            "{} not held with what is below it: {:?}",
            record.display(),
            denied.paths
        );
        assert!(
            !denied
                .paths
                .iter()
                .any(|it| it.access == Access::ReadWrite && record.starts_with(&it.path)),
            "{} is denied to reads",
            record.display()
        );
    }

    let mut compiled_harnesses = 0;
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        compiled_harnesses += 1;
        let applied = applied_for(
            harness,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(Os::MacOs),
        )
        .expect("compiles");
        match applied.form() {
            // Claude Code: its own sandbox's write denials, and its file tools'.
            Form::ClaudeCode(settings) => {
                let deny_write: Vec<&str> = settings.sandbox["filesystem"]["denyWrite"]
                    .as_array()
                    .expect("a list")
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                for path in &records {
                    let named = super::real(path).display().to_string();
                    assert!(deny_write.contains(&named.as_str()), "{named}");
                    assert!(
                        settings.deny.contains(&format!("Edit(/{named})")),
                        "{named}: {:?}",
                        settings.deny
                    );
                }
            }
            // Codex and opencode: purlis's own wrap, whose profile a chat at the root runs in,
            // on macOS and on Linux, whose supervisor judges each call by the same profile.
            Form::Codex(_) | Form::Opencode(_) => {
                for os in [Os::MacOs, Os::Linux] {
                    let applied = applied_for(
                        harness,
                        &policy_of(&[], false),
                        &Plane::of(None),
                        plane.path(),
                        &machine(os),
                    )
                    .expect("compiles");
                    let profile = seatbelt::profile(
                        applied.form().denied().expect("a wrap"),
                        &seatbelt::Own::default(),
                        plane.path(),
                        std::path::Path::new("/private/tmp/chat"),
                        &[4040],
                        None,
                    )
                    .expect("a profile");
                    let read = purlis_linux_sandbox::profile::Profile::parse(&profile)
                        .expect("read whole");
                    let done = |what, path: &std::path::Path, kind| {
                        read.allows(&purlis_linux_sandbox::profile::Op::File { what, path, kind })
                    };
                    use purlis_linux_sandbox::profile::{FileOp, Kind};
                    // The chat writes the project as it did.
                    assert!(done(
                        FileOp::Create,
                        &super::real(plane.path()).join("notes.txt"),
                        Kind::Regular
                    ));
                    for path in &records {
                        let path = super::real(path);
                        let mut spellings = vec![path.clone()];
                        // On macOS's case-folding disk, a record not there yet is made under the
                        // name as typed.
                        if os == Os::MacOs {
                            let name = path.file_name().expect("a name").to_string_lossy();
                            spellings.push(path.with_file_name(name.to_uppercase()));
                        }
                        for path in &spellings {
                            // The store as an entry: neither made, removed nor moved away…
                            for (what, kind) in [
                                (FileOp::Create, Kind::Directory),
                                (FileOp::Create, Kind::Regular),
                                (FileOp::Unlink, Kind::Directory),
                                (FileOp::Unlink, Kind::Regular),
                                (FileOp::WriteData, Kind::Regular),
                            ] {
                                assert!(
                                    !done(what, path, kind),
                                    "{harness:?} on {os:?}: {what:?} of {}",
                                    path.display()
                                );
                            }
                            // …and every record below it.
                            let below = path.join("01JZ0000000000000000000000.json");
                            for what in [FileOp::Create, FileOp::WriteData, FileOp::Unlink] {
                                assert!(
                                    !done(what, &below, Kind::Regular),
                                    "{harness:?} on {os:?}: {what:?} of {}",
                                    below.display()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(compiled_harnesses, 3, "Claude Code, Codex and opencode");
}

/// The Seatbelt rules that hold `path` with what is below it: under every spelling of its name
/// where `any_case`, as macOS holds everything in the state folder ([`kept_by_purlis`]), and as
/// spelled where not.
fn held_rules(path: &std::path::Path, access: Access, any_case: bool) -> Vec<String> {
    seatbelt::denial_rules(&Denial {
        class: Class::Integrity,
        path: path.to_path_buf(),
        access,
        named: None,
        reach: Reach::AndBelow,
        any_case,
    })
    .expect("rules")
}

/// purlis's older records in the state folder, as written out, each held from a chat's writes:
/// the app's own folder, the theme drafts, what skill deletes keep, the reports waiting for a
/// chat, the person's approvals and every chat's settings file.
const STATE_RECORDS: [&str; 9] = [
    "app",
    "theme-drafts",
    "skills-deleted",
    "skills-deleting",
    "handbacks",
    "harness-profiles-launched.json",
    "harness-declarations-approved.json",
    "mcp-approved.json",
    "chat-settings",
];

/// Those of [`STATE_RECORDS`] held where `$PURLIS_HOME` puts the state folder too.
const STATE_RECORDS_IN_PURLIS_HOME: [&str; 5] = [
    "theme-drafts",
    "skills-deleted",
    "skills-deleting",
    "mcp-approved.json",
    "chat-settings",
];

/// A machine of `os` whose `$PURLIS_HOME` is `state`, where one is given.
fn machine_with_state(os: Os, state: Option<&str>) -> Machine {
    let env: Vec<(&str, &str)> = state.map(|it| ("PURLIS_HOME", it)).into_iter().collect();
    Machine {
        env: crate::secrets::Env::of(&env),
        home: Some(std::path::PathBuf::from("/home/op")),
        os,
    }
}

/// **Every row purlis holds for its own state is held under every spelling on macOS**, and as
/// spelled on Linux, whose disks do not fold case: under both names of the state folder and
/// where `$PURLIS_HOME` puts it, with what waits for a chat's next turn. On a case-folding disk
/// a store not there yet is made under the name as typed, and purlis reads it under its own.
#[test]
fn every_row_purlis_holds_for_its_own_state_is_held_in_every_case_on_macos() {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::create_dir(plane.path().join(".purlis")).expect("a state folder");
    let elsewhere = plane.path().join("state-home");
    let elsewhere_named = elsewhere.display().to_string();
    for purlis_home in [None, Some(elsewhere_named.as_str())] {
        let mut states: Vec<std::path::PathBuf> = [".charter", ".purlis"]
            .iter()
            .map(|state| plane.path().join(state))
            .collect();
        if purlis_home.is_some() {
            states.push(elsewhere.clone());
        }
        for os in [Os::MacOs, Os::Linux] {
            let machine = machine_with_state(os, purlis_home);
            let denied = Denied::of(plane.path(), &machine).and_what_waits_for_a_chat(plane.path());
            let held: Vec<&Denial> = denied
                .paths
                .iter()
                .filter(|it| states.iter().any(|state| it.path.starts_with(state)))
                .collect();
            for state in &states[..2] {
                for record in STATE_RECORDS {
                    let path = state.join(record);
                    assert!(
                        held.iter().any(|it| it.path == path),
                        "{} is not held",
                        path.display()
                    );
                }
            }
            if purlis_home.is_some() {
                for record in STATE_RECORDS_IN_PURLIS_HOME {
                    let path = elsewhere.join(record);
                    assert!(
                        held.iter().any(|it| it.path == path),
                        "{} is not held",
                        path.display()
                    );
                }
            }
            for denial in &held {
                if os == Os::MacOs {
                    assert!(
                        denial.any_case,
                        "on macOS {} is held only as spelled",
                        denial.path.display()
                    );
                } else if denial.reach == Reach::AndBelow {
                    assert!(
                        !denial.any_case,
                        "on Linux {} is held in every case",
                        denial.path.display()
                    );
                }
            }
        }
    }
}

/// The same, as every harness's compiled sandbox holds it: **no chat makes one of purlis's
/// older state records under another spelling of its name on macOS**, nor the state folder
/// that is not there yet. Codex's and opencode's profile is read by the Linux supervisor's own
/// reading of it, on macOS and on Linux; Claude Code's denials name each in either case.
#[test]
fn every_harness_denies_a_chat_making_purlis_s_state_records_under_another_spelling() {
    let plane = tempfile::tempdir().expect("a plane");
    // `.purlis` is there and `.charter` is not, so both kinds of state folder are held.
    std::fs::create_dir(plane.path().join(".purlis")).expect("a state folder");
    let elsewhere = plane.path().join("state-home");
    let elsewhere_named = elsewhere.display().to_string();
    let mut records: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .flat_map(|state| STATE_RECORDS.map(|record| plane.path().join(state).join(record)))
        .collect();
    records.extend(STATE_RECORDS_IN_PURLIS_HOME.map(|record| elsewhere.join(record)));
    // Each record under the name as written and with its own name in upper case, and, for the
    // state folder that is not there, with the folder's name in upper case too.
    let spellings = |path: &std::path::Path| -> Vec<std::path::PathBuf> {
        let name = path.file_name().expect("a name").to_string_lossy();
        let mut out = vec![path.to_path_buf(), path.with_file_name(name.to_uppercase())];
        if path.parent().and_then(std::path::Path::file_name) == Some(".charter".as_ref()) {
            out.push(plane.path().join(".CHARTER").join(&*name));
        }
        out
    };

    let mut compiled_harnesses = 0;
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        compiled_harnesses += 1;
        for os in [Os::MacOs, Os::Linux] {
            let applied = applied_for(
                harness,
                &policy_of(&[], false),
                &Plane::of(None),
                plane.path(),
                &machine_with_state(os, Some(&elsewhere_named)),
            )
            .expect("compiles");
            match applied.form() {
                // Claude Code: its own sandbox's write denials, and its file tools', each in
                // either case on macOS.
                Form::ClaudeCode(settings) => {
                    if os != Os::MacOs {
                        continue;
                    }
                    let deny_write: Vec<&str> = settings.sandbox["filesystem"]["denyWrite"]
                        .as_array()
                        .expect("a list")
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect();
                    for path in &records {
                        let cased = claude::either_case(&super::real(path).display().to_string());
                        assert!(deny_write.contains(&cased.as_str()), "{cased} missing");
                        assert!(
                            settings.deny.contains(&format!("Edit(/{cased}/**)")),
                            "Edit(/{cased}/**) missing"
                        );
                    }
                    // What waits for a chat's next turn is kept from its reads too, in either
                    // case: its hooks run outside its sandbox.
                    let deny_read: Vec<&str> = settings.sandbox["filesystem"]["denyRead"]
                        .as_array()
                        .expect("a list")
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect();
                    for state in [".charter", ".purlis"] {
                        let path = super::real(&plane.path().join(state).join("handbacks"));
                        let cased = claude::either_case(&path.display().to_string());
                        assert!(deny_read.contains(&cased.as_str()), "{cased} read");
                        assert!(
                            settings.deny.contains(&format!("Read(/{cased}/**)")),
                            "Read(/{cased}/**) missing"
                        );
                    }
                }
                Form::Codex(_) | Form::Opencode(_) => {
                    let profile = seatbelt::profile(
                        applied.form().denied().expect("a wrap"),
                        &seatbelt::Own::default(),
                        plane.path(),
                        std::path::Path::new("/private/tmp/chat"),
                        &[4040],
                        None,
                    )
                    .expect("a profile");
                    let read = purlis_linux_sandbox::profile::Profile::parse(&profile)
                        .expect("read whole");
                    use purlis_linux_sandbox::profile::{FileOp, Kind};
                    let done = |what, path: &std::path::Path, kind| {
                        read.allows(&purlis_linux_sandbox::profile::Op::File { what, path, kind })
                    };
                    // The chat writes the project as it did.
                    assert!(done(
                        FileOp::Create,
                        &super::real(plane.path()).join("notes.txt"),
                        Kind::Regular
                    ));
                    for record in &records {
                        let record = super::real(record);
                        let held = if os == Os::MacOs {
                            spellings(&record)
                        } else {
                            vec![record.clone()]
                        };
                        for path in &held {
                            for kind in [Kind::Directory, Kind::Regular] {
                                assert!(
                                    !done(FileOp::Create, path, kind),
                                    "{harness:?} on {os:?}: {kind:?} made at {}",
                                    path.display()
                                );
                            }
                            let below = path.join("01JZ0000000000000000000000.json");
                            assert!(
                                !done(FileOp::Create, &below, Kind::Regular),
                                "{harness:?} on {os:?}: made {}",
                                below.display()
                            );
                        }
                    }
                    // The app's own records, which it reads back as its own, under another
                    // spelling of the app's folder on macOS.
                    let app = super::real(&plane.path().join(".purlis")).join("app");
                    let mut named = vec![app.clone()];
                    if os == Os::MacOs {
                        named.push(app.with_file_name("APP"));
                        named.push(app.with_file_name("App"));
                    }
                    for app in named {
                        for record in ["reopen.json", "app-state-id"] {
                            let path = app.join(record);
                            for what in [FileOp::Create, FileOp::WriteData] {
                                assert!(
                                    !done(what, &path, Kind::Regular),
                                    "{harness:?} on {os:?}: {what:?} of {}",
                                    path.display()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(compiled_harnesses, 3, "Claude Code, Codex and opencode");
}

/// D-T59-19: what waits to be told to a chat on its next turn (a task's report, purlis's word
/// on a held dispatch) is left by the app and taken by purlis's hooks. **Denied for reading
/// and writing where the harness's hooks run outside its sandbox, and left as it was where
/// they run inside it**: there the hook that delivers is held to the same sandbox, and a
/// denial would cut every report off.
#[test]
fn what_waits_for_a_chat_s_next_turn_is_denied_only_where_the_hooks_run_outside_the_sandbox() {
    let plane = tempfile::tempdir().expect("a plane");
    let stores: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .map(|state| plane.path().join(state).join("handbacks"))
        .collect();
    // Every harness is denied writing it: the app alone writes what a chat is handed. Reading
    // it is denied per harness.
    let classes = Denied::of(plane.path(), &machine(Os::MacOs));
    for store in &stores {
        assert!(
            classes
                .paths
                .iter()
                .any(|it| it.path == *store && it.access == Access::Write)
        );
        assert!(
            !classes
                .paths
                .iter()
                .any(|it| it.path == *store && it.access == Access::ReadWrite)
        );
    }
    let mut kinds = (0, 0);
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        let applied = applied_for(
            harness,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(Os::MacOs),
        )
        .expect("compiles");
        let denies = |store: &std::path::Path| {
            applied
                .denied
                .iter()
                .any(|it| it.path == *store && it.access == Access::ReadWrite)
        };
        if harness.adapter().sandbox_holds_what_it_starts() {
            kinds.1 += 1;
            for store in &stores {
                assert!(
                    !denies(store),
                    "{harness:?} runs its hooks inside its sandbox, and they read from {}",
                    store.display()
                );
                // Read, never written: the app takes what they read.
                assert!(
                    applied
                        .denied
                        .iter()
                        .any(|it| it.path == *store && it.access == Access::Write),
                    "{harness:?}: {} is not denied to writes",
                    store.display()
                );
            }
        } else {
            kinds.0 += 1;
            for store in &stores {
                assert!(denies(store), "{harness:?}: {} not denied", store.display());
            }
        }
    }
    assert!(
        kinds.0 > 0 && kinds.1 > 0,
        "both kinds are compiled: {kinds:?}"
    );
}

/// The messages between an asking chat and its tasks (#1442) wait in that same store, in
/// `said-<chat>/` beside the reports, so the one rule holds them too: denied with the reports
/// where the hooks run outside the sandbox, and reachable by the hook that takes them where
/// they run inside it (D-T59-19, D-T59-j14).
#[test]
fn the_messages_between_chats_wait_inside_the_store_that_denial_names() {
    let plane = tempfile::tempdir().expect("a plane");
    let message = crate::dispatchtalk::Message {
        kind: crate::dispatchtalk::Kind::FollowUp,
        from: "steward 3".to_owned(),
        chat: 3,
        text: "Also check staging.".to_owned(),
    };
    let file = crate::dispatchtalk::leave(plane.path(), 9, &message).expect("left");
    assert!(
        file.starts_with(crate::handback::dir(plane.path()).join("said-9")),
        "{}",
        file.display()
    );
    let denied =
        Denied::of(plane.path(), &machine(Os::MacOs)).and_what_waits_for_a_chat(plane.path());
    let held = paths(&denied, Class::Integrity, Access::ReadWrite);
    assert!(
        held.iter().any(|store| file.starts_with(store)),
        "{} is under none of {held:?}",
        file.display()
    );
    // And without that rule, as on a harness whose hooks run inside its sandbox, it is denied
    // to writes alone: the hook that reads the message there can still read it, and the app
    // takes it.
    let classes = Denied::of(plane.path(), &machine(Os::MacOs));
    assert!(
        !classes.paths.iter().any(|it| {
            it.reach == Reach::AndBelow
                && it.access == Access::ReadWrite
                && file.starts_with(&it.path)
        }),
        "{}",
        file.display()
    );
}

/// The same, in the form a harness whose hooks run outside is handed: its own sandbox's read
/// and write denials, and its file tools'.
#[test]
fn a_chat_whose_hooks_run_outside_is_handed_what_waits_denied_for_reading_and_writing() {
    let plane = tempfile::tempdir().expect("a plane");
    let denied =
        Denied::of(plane.path(), &machine(Os::MacOs)).and_what_waits_for_a_chat(plane.path());
    let stores: Vec<std::path::PathBuf> = [".charter", ".purlis"]
        .iter()
        .map(|state| plane.path().join(state).join("handbacks"))
        .collect();
    let held = paths(&denied, Class::Integrity, Access::ReadWrite);
    for store in &stores {
        assert!(held.contains(store), "{} not in {held:?}", store.display());
    }
    let settings = claude::settings(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    let listed = |key: &str| -> Vec<String> {
        settings.sandbox["filesystem"][key]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|path| path.as_str().map(str::to_owned))
            .collect()
    };
    for store in &stores {
        // As the kernel names it, which is what the compiler hands the harness.
        let named = super::real(store).display().to_string();
        assert!(listed("denyRead").contains(&named), "{named}");
        assert!(listed("denyWrite").contains(&named), "{named}");
        assert!(
            settings.deny.contains(&format!("Read(/{named}/**)")),
            "{named}: {:?}",
            settings.deny
        );
    }
}

#[test]
fn a_chat_never_writes_the_approvals_a_person_gave() {
    let (_plane, denied) = denied_with(None, Os::Linux);
    assert_eq!(
        paths(&denied, Class::HumanPowers, Access::Write),
        // Under both names, there or not: whichever is there is the config home (RN-5).
        // The browser engines purlis downloads, in the data home in use and under the default
        // data folder's other name (#2835). The harnesses a runner installs, in the data home in
        // use (#3446).
        // …and this machine's policy (#1343), as the kernel names it.
        [
            std::path::PathBuf::from("/home/op/.config/purlis"),
            std::path::PathBuf::from("/home/op/.config/charter"),
            std::path::PathBuf::from("/home/op/.local/share/charter/engines/chrome-for-testing"),
            std::path::PathBuf::from("/home/op/.local/share/purlis/engines/chrome-for-testing"),
            std::path::PathBuf::from("/home/op/.local/share/charter/harnesses"),
            super::real(&super::policy::machine_folder()),
        ]
    );
}

#[test]
fn a_chat_never_reads_or_writes_the_forge_answers_the_humans_token_fetched() {
    // The native forge transport's ETag store holds raw answers the human's sign-in token
    // fetched (ADR 0070 §3, FW-2a); a chat reads neither it nor the bodies in it.
    let (_plane, denied) = denied_with(None, Os::Linux);
    assert_eq!(
        paths(&denied, Class::HumanPowers, Access::ReadWrite),
        [
            // The human client scopes' credentials (FD-27), under every name the folder and
            // the config home have (RN-5).
            std::path::PathBuf::from("/home/op/.config/purlis/purlisd"),
            std::path::PathBuf::from("/home/op/.config/purlis/charterd"),
            // A runner's one peer (ADR 0078 §8, #2044).
            std::path::PathBuf::from("/home/op/.config/purlis/peers.json"),
            // The runners this desktop pinned (#2525).
            std::path::PathBuf::from("/home/op/.config/purlis/runners.json"),
            // When each of them ran (#3492).
            std::path::PathBuf::from("/home/op/.config/purlis/runner-hours.jsonl"),
            // Each cluster pool's own copy of its kubeconfig's context (#3292).
            std::path::PathBuf::from("/home/op/.config/purlis/cluster-kubeconfigs"),
            std::path::PathBuf::from("/home/op/.config/purlis/forge-etags"),
            std::path::PathBuf::from("/home/op/.config/charter/purlisd"),
            std::path::PathBuf::from("/home/op/.config/charter/charterd"),
            std::path::PathBuf::from("/home/op/.config/charter/peers.json"),
            std::path::PathBuf::from("/home/op/.config/charter/runners.json"),
            std::path::PathBuf::from("/home/op/.config/charter/runner-hours.jsonl"),
            std::path::PathBuf::from("/home/op/.config/charter/cluster-kubeconfigs"),
            std::path::PathBuf::from("/home/op/.config/charter/forge-etags"),
            // This machine's profiles' folders (#2034).
            Homes::charter_data(&machine(Os::Linux))
                .expect("a data home")
                .join(crate::profilestore::DIR),
            // The data folders of the browsers the session host runs (#2012).
            Homes::charter_data(&machine(Os::Linux))
                .expect("a data home")
                .join(crate::datahome::BROWSERS),
            // The checked copies of Docker's settings each run of `docker` is given (#2752).
            Homes::charter_data(&machine(Os::Linux))
                .expect("a data home")
                .join(crate::dockerpool::settings::DIR),
            // The browser logins this device keeps (#2021).
            Homes::charter_data(&machine(Os::Linux))
                .expect("a data home")
                .join(crate::browserlogins::DIR),
            // The session host's socket folder (#1997), where the host itself takes it.
            crate::hostsocket::socket_for(None)
                .parent()
                .expect("a folder")
                .to_path_buf(),
        ]
    );
}

/// **On a runner, a chat reads no other profile's folder** (#2049): a runner keeps each of its
/// desktop's profiles under its own `<data>/profiles`, each with its sign-in, and its chats are
/// held by the Linux sandbox, which denies the whole folder as on any machine.
#[test]
fn on_linux_no_chat_reads_or_writes_the_folders_of_the_profiles_kept_there() {
    let profiles = Homes::charter_data(&machine(Os::Linux))
        .expect("a data home")
        .join(crate::profilestore::DIR);
    let (_plane, denied) = denied_with(None, Os::Linux);
    assert!(
        paths(&denied, Class::HumanPowers, Access::ReadWrite).contains(&profiles),
        "{:?}",
        denied.paths
    );
}

#[test]
fn no_chat_reads_or_writes_the_folders_of_this_machines_profiles() {
    // A profile purlis made keeps its sign-in in its folder (ADR 0095 §1, #2034).
    let profiles = Homes::charter_data(&machine(Os::MacOs))
        .expect("a data home")
        .join(crate::profilestore::DIR);
    let (_plane, denied) = denied_with(None, Os::MacOs);
    assert!(
        paths(&denied, Class::HumanPowers, Access::ReadWrite).contains(&profiles),
        "{:?}",
        denied.paths
    );
    let named = super::real(&profiles).display().to_string();
    let settings = claude::settings(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    for key in ["denyRead", "denyWrite"] {
        assert!(
            settings.sandbox["filesystem"][key]
                .as_array()
                .is_some_and(|all| all.iter().any(|p| p == named.as_str())),
            "{key}: {}",
            settings.sandbox["filesystem"][key]
        );
    }
    for (harness, wrapped) in [
        (
            "codex",
            codex::wrap(&compiled(denied.clone(), Os::MacOs))
                .expect("compiles")
                .denied,
        ),
        (
            "opencode",
            opencode::wrap(&compiled(denied, Os::MacOs))
                .expect("compiles")
                .denied,
        ),
    ] {
        assert!(
            wrapped
                .iter()
                .any(|it| it.path == profiles && it.access == Access::ReadWrite),
            "{harness}: {wrapped:?}"
        );
    }
}

#[test]
fn no_chat_writes_the_browser_engines_purlis_downloads_under_either_name_of_the_data_home() {
    // The AppArmor profile the `.deb` installs lets a program at the pinned engine's path make
    // user namespaces (#2835), so no chat may put a program there: not under the data home in
    // use, nor under its other name, which the profile names too.
    let linux = machine(Os::Linux);
    let data = Homes::charter_data(&linux).expect("a data home");
    let share = linux.home.as_ref().expect("a home").join(".local/share");
    let (_plane, denied) = denied_with(None, Os::Linux);
    let written = paths(&denied, Class::HumanPowers, Access::Write);
    for engines in [
        crate::browsers::engines_in(&data),
        crate::browsers::engines_in(&share.join("purlis")),
        crate::browsers::engines_in(&share.join("charter")),
    ] {
        assert!(
            written.contains(&engines),
            "{engines:?}: {:?}",
            denied.paths
        );
    }
}

#[test]
fn no_chat_writes_the_harnesses_a_runner_installs() {
    // A runner's chat runs the harness version its profile pins, installed in the data home
    // (#3446): a chat that could write it would choose the program the next chat runs.
    let linux = machine(Os::Linux);
    let harnesses = Homes::charter_data(&linux)
        .expect("a data home")
        .join(crate::profilestore::installs::DIR);
    let (_plane, denied) = denied_with(None, Os::Linux);
    assert!(
        paths(&denied, Class::HumanPowers, Access::Write).contains(&harnesses),
        "{:?}",
        denied.paths
    );
}

#[test]
fn no_chat_reads_or_writes_the_data_folders_of_the_browsers_the_session_host_runs() {
    // A running browser's data folder holds its engine's profile: the cookies and storage of
    // every site the person opens in it (#2012). A chat neither reads nor writes any of them.
    let browsers = Homes::charter_data(&machine(Os::MacOs))
        .expect("a data home")
        .join(crate::datahome::BROWSERS);
    let (_plane, denied) = denied_with(None, Os::MacOs);
    assert!(
        paths(&denied, Class::HumanPowers, Access::ReadWrite).contains(&browsers),
        "{:?}",
        denied.paths
    );
    let named = super::real(&browsers).display().to_string();
    let settings = claude::settings(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    for key in ["denyRead", "denyWrite"] {
        assert!(
            settings.sandbox["filesystem"][key]
                .as_array()
                .is_some_and(|all| all.iter().any(|p| p == named.as_str())),
            "{key}: {}",
            settings.sandbox["filesystem"][key]
        );
    }
    for (harness, wrapped) in [
        (
            "codex",
            codex::wrap(&compiled(denied.clone(), Os::MacOs))
                .expect("compiles")
                .denied,
        ),
        (
            "opencode",
            opencode::wrap(&compiled(denied, Os::MacOs))
                .expect("compiles")
                .denied,
        ),
    ] {
        assert!(
            wrapped
                .iter()
                .any(|it| it.path == browsers && it.access == Access::ReadWrite),
            "{harness}: {wrapped:?}"
        );
    }
}

#[test]
fn no_chat_reads_or_writes_the_browser_logins_this_device_keeps() {
    // A kept browser login is a signed-in browser's own data, sealed (#2021): a chat neither
    // reads nor writes it, so none takes, swaps or puts an older copy back.
    let browsers = Homes::charter_data(&machine(Os::MacOs))
        .expect("a data home")
        .join(crate::browserlogins::DIR);
    let (_plane, denied) = denied_with(None, Os::MacOs);
    assert!(
        paths(&denied, Class::HumanPowers, Access::ReadWrite).contains(&browsers),
        "{:?}",
        denied.paths
    );
    let named = super::real(&browsers).display().to_string();
    let settings = claude::settings(&compiled(denied.clone(), Os::MacOs)).expect("compiles");
    for key in ["denyRead", "denyWrite"] {
        assert!(
            settings.sandbox["filesystem"][key]
                .as_array()
                .is_some_and(|all| all.iter().any(|p| p == named.as_str())),
            "{key}: {}",
            settings.sandbox["filesystem"][key]
        );
    }
    for (harness, wrapped) in [
        (
            "codex",
            codex::wrap(&compiled(denied.clone(), Os::MacOs))
                .expect("compiles")
                .denied,
        ),
        (
            "opencode",
            opencode::wrap(&compiled(denied, Os::MacOs))
                .expect("compiles")
                .denied,
        ),
    ] {
        assert!(
            wrapped
                .iter()
                .any(|it| it.path == browsers && it.access == Access::ReadWrite),
            "{harness}: {wrapped:?}"
        );
    }
}

#[test]
fn a_chats_sandbox_finds_the_data_home_where_the_profile_store_does() {
    // A relative `$PURLIS_DATA_HOME` is taken from the directory purlis was started in, by the
    // store and the sandbox alike, so the folder a chat is denied is the one profiles are in.
    let machine = mac_with(&[(crate::datahome::HOME_VAR, "relative/data")]);
    let env = |name: &str| machine.env.get(name);
    assert_eq!(
        Homes::charter_data(&machine),
        crate::datahome::root_in(&env)
    );
    assert_eq!(
        Homes::charter_data(&machine),
        std::path::absolute("relative/data").ok()
    );
}

#[test]
fn claude_code_and_codex_both_deny_a_chat_the_forge_etag_store() {
    let etags = "/home/op/.config/charter/forge-etags";
    let (_plane, denied) = denied_with(None, Os::Linux);
    let settings = claude::settings(&compiled(denied.clone(), Os::Linux)).expect("compiles");
    let read_denied = settings.sandbox["filesystem"]["denyRead"].clone();
    assert!(
        read_denied
            .as_array()
            .is_some_and(|all| all.iter().any(|p| p == etags)),
        "{read_denied}"
    );
    assert!(
        settings.deny.contains(&format!("Read(/{etags}/**)")),
        "{:?}",
        settings.deny
    );
    let wrap = codex::wrap(&compiled(denied, Os::MacOs)).expect("compiles");
    assert!(
        wrap.denied
            .iter()
            .any(|it| it.path == std::path::Path::new(etags) && it.access == Access::ReadWrite),
        "{:?}",
        wrap.denied
    );
}

#[test]
fn off_a_runner_there_are_no_runner_internals_to_deny() {
    // A placeholder for class 4, kept because ADR 0067 fixes the classes: charter has no
    // runner yet (RR-5), so there is nothing of one to deny. The runner slice replaces this
    // with the paths it installs, denied on a runner.
    let (_plane, denied) = denied_with(None, Os::Linux);
    assert!(
        denied
            .paths
            .iter()
            .all(|it| it.class != Class::RunnerInternals)
    );
}

// -------------------------------------------------------------------------------------
// Egress: named presets, and the plane's own forge hosts
// -------------------------------------------------------------------------------------

fn hosts_of(presets: &[Preset], plane: Option<&str>) -> Vec<String> {
    hosts(presets, &Plane::of(plane), &policy::Locks::none())
}

#[test]
fn each_preset_has_the_name_settings_shows_it_by() {
    let titles: Vec<&str> = Preset::ALL.iter().map(|preset| preset.title()).collect();
    assert_eq!(
        titles,
        ["AI providers", "Code hosting", "Package registries"]
    );
}

#[test]
fn settings_counts_the_hosts_a_chat_is_granted_besides_its_presets() {
    let host = |written: &str| hosts::Host::parse(written).expect("a host");
    let counted = counted_hosts(&hosts::off_this_machine(
        hosts::in_force(
            &[host("api.example.com"), host("10.0.0.5")],
            &[host("api.example.com"), host("tools.example.org")],
            &[],
            &hosts::Locks::default(),
        ),
        &["10.0.0.5".parse().expect("an address")],
    ));
    // This machine's own address reaches nothing, and a host yours repeats is the project's.
    assert_eq!(
        counted,
        Besides {
            project_hosts: 1,
            your_hosts: 1,
            folders: 0,
        }
    );
}

/// #1340: what Settings counts is what a chat on no persona is granted as it starts, so the
/// sentence and a start never drift apart. Both read [`granted_hosts`] and [`granted_folders`];
/// a folder grant cannot be compiled from a fixture in a temp folder (D-1342-14), so the
/// folders compare at none here.
#[test]
fn settings_counts_what_a_chat_with_no_persona_is_granted() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("the project");
    let plane = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example.com\", \"tools.example.org\"]\n",
    ));
    let policy = plane.said().policy.expect("on");
    let machine = Machine::this();

    let counted = besides(&root, &plane, &machine);
    let chat = Compiled::granted(
        &policy,
        &plane,
        &root,
        &machine,
        None,
        &grant::Grants::default(),
    );

    assert_eq!(
        counted.project_hosts + counted.your_hosts,
        chat.hosts.len(),
        "{:?}",
        chat.hosts
    );
    assert_eq!(counted.project_hosts, 2);
    assert_eq!(counted.folders, chat.writable.len());
}

#[test]
fn a_project_without_the_sandbox_has_nothing_besides_its_presets() {
    let root = tempfile::tempdir().expect("a project");
    assert_eq!(
        besides(root.path(), &Plane::of(None), &Machine::this()),
        Besides::default()
    );
}

#[test]
fn the_forge_preset_adds_the_self_managed_hosts_the_plane_tracks() {
    let hosts = hosts_of(
        &[Preset::Forge],
        Some("[[forge]]\nkind = \"gitlab\"\nhost = \"git.example.org:8443\"\n"),
    );
    assert!(hosts.contains(&"github.com".to_owned()), "{hosts:?}");
    assert!(hosts.contains(&"gitlab.com".to_owned()), "{hosts:?}");
    assert!(hosts.contains(&"git.example.org".to_owned()), "{hosts:?}");
    assert!(
        !hosts.contains(&"api.anthropic.com".to_owned()),
        "{hosts:?}"
    );
}

#[test]
fn a_forge_host_that_is_not_a_host_is_never_let_through() {
    let hosts = hosts_of(&[Preset::Forge], Some("[[forge]]\nhost = \"*\"\n"));
    assert!(!hosts.contains(&"*".to_owned()), "{hosts:?}");
}

#[test]
fn no_preset_is_no_host() {
    assert_eq!(hosts_of(&[], None), Vec::<String>::new());
}

#[test]
fn the_model_provider_preset_reaches_the_three_harnesses_providers() {
    let hosts = hosts_of(&[Preset::ModelProviders], None);
    for host in ["api.anthropic.com", "api.openai.com", "opencode.ai"] {
        assert!(hosts.contains(&host.to_owned()), "{host}: {hosts:?}");
    }
}

/// What each harness's compiled sandbox lets a chat reach, as its compiler hands it on: Claude
/// Code's `network.allowedDomains`, and the egress proxy's list for a harness purlis wraps.
fn reached(applied: &Applied) -> Vec<String> {
    match applied.form() {
        Form::ClaudeCode(settings) => settings.sandbox["network"]["allowedDomains"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|host| host.as_str().expect("text").to_owned())
            .collect(),
        Form::Codex(wrap) => wrap.hosts.clone(),
        Form::Opencode(wrap) => wrap.hosts.clone(),
    }
}

/// #1341: the project's own hosts reach every chat in it with no approval, on every harness,
/// private addresses and ports included; this machine's are added to them, and a host that is
/// not one grants nothing.
#[test]
fn the_project_s_hosts_and_this_machine_s_reach_a_chat_on_every_harness() {
    let plane = plane_saying(
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"10.100.39.145:6443\", \
         \"*.internal.example\", \"169.254.169.254\"]\n",
    );
    std::fs::write(
        plane.path().join("charter.local.toml"),
        "[sandbox]\nhosts = [\"[fd00::7]:8443\", \"10.100.39.145:6443\"]\n",
    )
    .expect("charter.local.toml");
    // Confirmed in Settings on this machine, as adding it there does.
    local::confirm_host(plane.path(), "[fd00::7]:8443").expect("confirmed");
    for harness in Harness::ALL {
        let applied =
            compiled_anyway(harness, plane.path(), &machine(Os::MacOs)).expect("compiles");
        assert_eq!(
            reached(&applied),
            ["10.100.39.145:6443", "*.internal.example", "[fd00::7]:8443"],
            "{harness:?}"
        );
    }
}

/// #1341: another project's hosts, and this machine's file in another project, reach nothing
/// here: hosts are the project's and this machine's for this project.
#[test]
fn a_project_without_hosts_reaches_only_its_presets() {
    let plane = plane_saying("[sandbox]\nmode = \"on\"\negress = []\n");
    let other = plane_saying("[sandbox]\nmode = \"on\"\nhosts = [\"10.0.0.5\"]\n");
    std::fs::write(
        other.path().join("charter.local.toml"),
        "[sandbox]\nhosts = [\"10.0.0.6\"]\n",
    )
    .expect("charter.local.toml");
    for harness in Harness::ALL {
        let applied =
            compiled_anyway(harness, plane.path(), &machine(Os::MacOs)).expect("compiles");
        assert_eq!(reached(&applied), Vec::<String>::new(), "{harness:?}");
    }
}

/// #1341: this machine's file grants nothing once git would carry it, since what it says
/// would then reach every clone with no trace of the project's flow.
#[test]
fn this_machine_s_hosts_in_a_file_git_would_commit_grant_nothing() {
    let plane = plane_saying("[sandbox]\nmode = \"on\"\negress = []\n");
    crate::testgit::run(plane.path(), &["init", "-q"]);
    std::fs::write(
        plane.path().join("charter.local.toml"),
        "[sandbox]\nhosts = [\"10.0.0.6\"]\n",
    )
    .expect("charter.local.toml");
    let applied =
        compiled_anyway(Harness::ClaudeCode, plane.path(), &machine(Os::MacOs)).expect("compiles");
    assert_eq!(reached(&applied), Vec::<String>::new());
}

// -------------------------------------------------------------------------------------
// Claude Code: the `sandbox` object in the `--settings` charter already passes
// -------------------------------------------------------------------------------------

fn compiled(denied: Denied, os: Os) -> Compiled {
    Compiled {
        level: Level::Full,
        held: Vec::new(),
        denied,
        hosts: vec!["github.com".to_owned()],
        reach: reach::Reach::open(vec!["github.com".to_owned()]),
        writable: Vec::new(),
        os,
        homes: Homes::default(),
        widened: Widened::default(),
    }
}

fn one(class: Class, path: &str, access: Access) -> Denial {
    Denial {
        class,
        path: std::path::PathBuf::from(path),
        access,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    }
}

#[test]
fn a_claude_code_chat_is_sandboxed_with_no_way_out_and_no_silent_fallback() {
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    assert_eq!(settings.sandbox["enabled"], true);
    assert_eq!(settings.sandbox["allowUnsandboxedCommands"], false);
    assert_eq!(settings.sandbox["failIfUnavailable"], true);
}

/// On Linux, at the full sandbox and at the floor, a Claude Code chat whose own sandbox cannot
/// start (on a runner, where only the browser engine may make the namespaces bubblewrap needs,
/// D-2050-25) is refused, never run without it.
#[test]
fn on_linux_a_claude_code_chat_whose_sandbox_cannot_start_is_refused_at_every_level() {
    for level in [Level::Full, Level::Floor] {
        let settings = claude::settings(&Compiled {
            level,
            ..compiled(Denied::default(), Os::Linux)
        })
        .expect("compiles");
        assert_eq!(settings.sandbox["enabled"], true, "{level:?}");
        assert_eq!(settings.sandbox["failIfUnavailable"], true, "{level:?}");
        assert_eq!(
            settings.sandbox["allowUnsandboxedCommands"], false,
            "{level:?}"
        );
    }
}

#[test]
fn a_claude_code_chat_reaches_only_the_presets_hosts_and_is_never_asked_to_widen_them() {
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    assert_eq!(
        settings.sandbox["network"],
        serde_json::json!({
            "allowedDomains": ["github.com"],
            "strictAllowlist": true,
            "allowAllUnixSockets": false,
            "allowLocalBinding": false,
        })
    );
}

#[test]
fn a_sandboxed_claude_code_chat_offers_no_browser_tools_of_its_own() {
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    assert!(
        settings
            .deny
            .iter()
            .any(|rule| rule == "mcp__claude-in-chrome"),
        "{:?}",
        settings.deny
    );
}

#[test]
fn claude_codes_web_tools_are_denied_because_the_allowed_hosts_do_not_hold_them() {
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    assert_eq!(settings.deny[..2], ["WebFetch", "WebSearch"]);
    // Then its own browser's tools (#2013), and then the paths.
    assert_eq!(settings.deny[2], "mcp__claude-in-chrome");
}

/// `rules` without the later-code class's, which name a path at any depth (`**/`).
fn by_path(rules: &[String]) -> Vec<String> {
    rules
        .iter()
        .filter(|rule| !rule.contains("**/"))
        .cloned()
        .collect()
}

/// A Claude Code sandbox's `denyRead` and `denyWrite`, each without the later-code class's.
fn filesystem_by_path(settings: &claude::Settings) -> (Vec<String>, Vec<String>) {
    let list = |key: &str| -> Vec<String> {
        settings.sandbox["filesystem"][key]
            .as_array()
            .expect("a list")
            .iter()
            .map(|it| it.as_str().expect("a path").to_owned())
            .collect()
    };
    (by_path(&list("denyRead")), by_path(&list("denyWrite")))
}

#[test]
fn a_path_denied_to_read_is_denied_to_the_sandbox_and_to_claude_codes_own_tools() {
    let denied = Denied {
        paths: vec![one(Class::Vaults, "/p/.charter/vaults", Access::ReadWrite)],
        services: vec![],
        unread: None,
        untold: None,
        git_folder_pins: vec![],
    };
    let settings = claude::settings(&compiled(denied, Os::Linux)).expect("compiles");
    assert_eq!(
        filesystem_by_path(&settings),
        (
            vec!["/p/.charter/vaults".to_owned()],
            vec!["/p/.charter/vaults".to_owned()]
        )
    );
    assert_eq!(
        by_path(&settings.deny)[3..],
        [
            "Read(//p/.charter/vaults)",
            "Read(//p/.charter/vaults/**)",
            "Edit(//p/.charter/vaults)",
            "Edit(//p/.charter/vaults/**)",
        ]
    );
}

#[test]
fn a_path_denied_to_write_stays_readable() {
    let denied = Denied {
        paths: vec![one(Class::Integrity, "/p/.charter/app", Access::Write)],
        services: vec![],
        unread: None,
        untold: None,
        git_folder_pins: vec![],
    };
    let settings = claude::settings(&compiled(denied, Os::Linux)).expect("compiles");
    assert_eq!(
        filesystem_by_path(&settings),
        (Vec::new(), vec!["/p/.charter/app".to_owned()])
    );
    assert_eq!(
        by_path(&settings.deny)[3..],
        ["Edit(//p/.charter/app)", "Edit(//p/.charter/app/**)"]
    );
}

/// A Claude Code sandbox's `denyWrite`, the later-code class's included.
fn deny_write(settings: &claude::Settings) -> Vec<String> {
    settings.sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|it| claude::uncased(it.as_str().expect("a path")))
        .collect()
}

#[test]
fn a_claude_code_chat_is_held_from_writing_every_later_code_name_at_any_depth() {
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    let deny_write = deny_write(&settings);
    let held = |glob: &str| {
        deny_write.iter().any(|it| it == glob)
            && settings
                .deny
                .iter()
                .any(|it| *it == format!("Edit({glob})"))
            && settings
                .deny
                .iter()
                .any(|it| *it == format!("Edit({glob}/**)"))
    };
    // Every name the class holds with what is below it, for the commands Claude Code's sandbox
    // runs and for its own Edit and Write tools. A name that holds only itself (`.git`) is the
    // stated gap #1065: no glob here denies it without denying what git writes below it.
    for planted in PLANTED {
        let glob = format!("**/{}", planted.path);
        match planted.reach {
            Reach::AndBelow => assert!(held(&glob), "{glob} is not held: {settings:?}"),
            Reach::Itself => assert!(!deny_write.contains(&glob), "{glob}"),
        }
    }
    // Each harness's project config that can start code or change a server's command or
    // environment at its next launch (#1057), written out.
    for glob in [
        "**/.mcp.json",
        "**/.claude/settings.json",
        "**/.claude/settings.local.json",
        "**/.claude/commands",
        "**/.claude/agents",
        "**/.claude/skills",
        "**/opencode.json",
        "**/opencode.jsonc",
        "**/.opencode",
        "**/tui.json",
        "**/tui.jsonc",
        "**/.codex",
        "**/.agents",
    ] {
        assert!(held(glob), "{glob} is not held");
    }
}

#[test]
fn on_macos_the_store_holds_the_credential_store_so_a_claude_code_chat_compiles() {
    // Ruling V90a: charter writes every keyring item so that only its own app reads it without
    // the person's confirmation, so the vaults class is held by the store for every harness.
    let denied = Denied {
        paths: vec![],
        services: vec![Service::CredentialStore],
        unread: None,
        untold: None,
        git_folder_pins: vec![],
    };
    claude::settings(&compiled(denied, Os::MacOs)).expect("compiles on macOS");
}

#[test]
fn on_linux_claude_code_cannot_hold_the_credential_store_so_the_chat_does_not_start() {
    // The Secret Service keeps no per-program rule, and Claude Code's own sandbox has not been
    // measured keeping a command off the session bus (ruling V90c).
    let denied = Denied {
        paths: vec![],
        services: vec![Service::CredentialStore],
        unread: None,
        untold: None,
        git_folder_pins: vec![],
    };
    let refused = claude::settings(&compiled(denied, Os::Linux)).expect_err("refused");
    assert_eq!(refused.class(), Some(Class::Vaults));
}

// -------------------------------------------------------------------------------------
// Codex: what a chat's own words may not say to the Codex charter wraps
// -------------------------------------------------------------------------------------

fn words(line: &str) -> Vec<String> {
    line.split(' ').map(str::to_owned).collect()
}

#[test]
fn a_codex_command_that_would_drop_or_widen_the_sandbox_is_named() {
    // Each measured on codex-cli 0.147.0 or read from its source: `-s` of any value and the
    // bypass drop the profile whole, `--add-dir` and `--cd` move what is writable, `-a` and
    // `--approve-for-me` outrank the approval policy, `--search` turns the web search back on,
    // and a feature flag's effect on the sandbox is unmeasured, one feature at a time.
    for (line, flag) in [
        ("codex -s danger-full-access", "-s"),
        ("codex -s read-only", "-s"),
        ("codex --sandbox=workspace-write", "--sandbox"),
        ("codex -sdanger-full-access", "-s"),
        (
            "codex --dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-approvals-and-sandbox",
        ),
        ("codex --yolo", "--yolo"),
        ("codex --add-dir /home/op", "--add-dir"),
        ("codex -C /home/op", "-C"),
        ("codex --cd=/home/op", "--cd"),
        ("codex -a on-request", "-a"),
        ("codex --ask-for-approval untrusted", "--ask-for-approval"),
        ("codex --approve-for-me", "--approve-for-me"),
        ("codex --not-so-yolo", "--not-so-yolo"),
        ("codex --search", "--search"),
        ("codex --disable network_proxy", "--disable"),
        ("codex --disable=network_proxy", "--disable"),
        ("codex --enable browser_use", "--enable"),
        ("codex --enable=respect_system_proxy", "--enable"),
    ] {
        let why = codex::loosened_by(&words(line)).unwrap_or_else(|| panic!("{line}"));
        assert!(why.contains(&format!("`{flag}`")), "{line}: {why}");
    }
}

#[test]
fn a_codex_config_override_outside_the_few_that_cannot_touch_the_sandbox_is_named() {
    // Each class a `-c` could reach the sandbox through, in each spelling Codex takes.
    for key in [
        "sandbox_mode=\"danger-full-access\"",
        "sandbox_workspace_write.network_access=true",
        "profile=\"loose\"",
        "default_permissions=\":danger-full-access\"",
        "permissions.mine.network.enabled=true",
        "features.network_proxy=false",
        "tools.web_search=true",
        "approval_policy=\"on-request\"",
        "approvals_reviewer=\"auto_review\"",
        "web_search=\"live\"",
        "network.enabled=true",
        "hooks.Stop=[]",
        "mcp_servers.x.command=\"sh\"",
        "shell_environment_policy.inherit=\"all\"",
    ] {
        let name = key.split('=').next().expect("a key");
        for line in [
            format!("codex -c {key}"),
            format!("codex --config {key}"),
            format!("codex --config={key}"),
            format!("codex -c{key}"),
        ] {
            let why = codex::loosened_by(&words(&line)).unwrap_or_else(|| panic!("{line}"));
            assert!(why.contains(&format!("`-c {name}`")), "{line}: {why}");
        }
    }
}

#[test]
fn a_codex_command_that_only_tightens_or_does_not_touch_the_sandbox_is_not_named() {
    for line in [
        "codex",
        "codex -a never",
        "codex --ask-for-approval=never",
        // Measured: a `-p` profile's `sandbox_mode`, `default_permissions`,
        // `sandbox_workspace_write`, approval policy, web search and proxy feature all lose to
        // charter's flags.
        "codex -m gpt-5 -p work",
        "codex resume 0199",
        "codex -c model=\"o3\"",
        "codex --config model_reasoning_effort=\"high\"",
        "codex -c model_providers.local.base_url=\"http://localhost:1234/v1\"",
    ] {
        assert_eq!(codex::loosened_by(&words(line)), None, "{line}");
    }
    // A first message is not a flag, even one that starts with a dash.
    assert_eq!(codex::loosened_by(&["-a quick fix".to_owned()]), None);
}

/// The arguments `applied` gives a chat with these words, or why it may not start.
fn args_of(
    applied: &Applied,
    command: Vec<String>,
    armed: Vec<String>,
    charters: Vec<String>,
) -> Result<Vec<String>, String> {
    applied
        .line(
            Words {
                program: "codex".to_owned(),
                command,
                armed,
                charters,
            },
            &At {
                cwd: Some(applied.root()),
                ..At::default()
            },
        )
        .map(|line| line.args)
}

/// What a Codex chat starts under in `plane`, wrapped (#1123).
fn codex_sandbox_in(plane: &tempfile::TempDir) -> Applied {
    for_start(Harness::Codex, plane.path(), &machine(Os::MacOs), &|_| true)
        .expect("starts")
        .expect("sandboxed")
}

#[test]
fn a_sandbox_is_refused_a_line_that_would_drop_it_and_says_where_the_flag_came_from() {
    let plane = plane_saying(ON);
    let applied = codex_sandbox_in(&plane);
    assert_eq!(
        args_of(
            &applied,
            words("-s danger-full-access"),
            Vec::new(),
            Vec::new()
        ),
        Err(
            "this project runs every chat sandboxed, and the profile's command names `-s`, which \
             would run Codex outside the sandbox purlis compiled for it, so nothing was \
             started. Take it out of the profile's command."
                .to_owned()
        )
    );
    assert_eq!(
        args_of(
            &applied,
            Vec::new(),
            Vec::new(),
            words("-c sandbox_mode=\"danger-full-access\"")
        ),
        Err(
            "this project runs every chat sandboxed, and the chat's own arguments name \
             `-c sandbox_mode`, which would run Codex outside the sandbox purlis compiled for \
             it, so nothing was started. Start it without that argument."
                .to_owned()
        )
    );
}

// -------------------------------------------------------------------------------------
// The start: sandboxed, or not started (fail closed)
// -------------------------------------------------------------------------------------

fn plane_saying(toml: &str) -> tempfile::TempDir {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), toml).expect("charter.toml");
    plane
}

fn machine(os: Os) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os,
    }
}

const ON: &str = "[sandbox]\nmode = \"on\"\n";

/// [`ON`] without the `toolchains` preset, for a test whose machine has no data home to make the
/// project's package caches in (#1337): a start there is refused, naming the folder.
const ON_WITHOUT_CACHES: &str =
    "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

#[test]
fn a_chat_in_a_plane_that_says_nothing_starts_at_the_floor_or_as_before_where_it_cannot() {
    // #2071: the floor is the default. Where it cannot be applied (no backend here), the chat
    // starts as it always has, and says why.
    let plane = plane_saying("schema = 1\n");
    let decided = decide(
        Harness::Codex,
        plane.path(),
        &machine(Os::Windows),
        &|_| false,
        None,
        None,
    );
    assert!(
        matches!(decided, Ok(Some(Decided::Floorless(_)))),
        "{decided:?}"
    );
}

#[test]
fn a_claude_code_chat_in_a_sandboxed_plane_starts_sandboxed_for_claude_code() {
    let plane = plane_saying(ON);
    let applied = for_start(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| true,
    )
    .expect("starts")
    .expect("sandboxed");
    assert_eq!(applied.harness(), Harness::ClaudeCode);
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!("compiled for Claude Code: {:?}", applied.form());
    };
    assert_eq!(settings.sandbox["enabled"], true);
}

#[test]
fn a_mistyped_mode_does_not_start_a_chat_unsandboxed() {
    let plane = plane_saying("[sandbox]\nmode = \"of\"\n");
    let started = for_start(
        Harness::Opencode,
        plane.path(),
        &machine(Os::Linux),
        &|_| true,
    );
    assert!(
        matches!(
            started,
            Ok(Some(ref applied))
                if applied.harness() == Harness::Opencode && applied.level() == Level::Full
        ),
        "{started:?}"
    );
}

#[test]
fn a_linux_machine_without_socat_is_told_which_program_and_how_to_install_it() {
    let plane = plane_saying(ON);
    let refused = for_start(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|program| program == "bwrap",
    )
    .expect_err("refused");
    assert_eq!(
        refused.to_string(),
        "this project runs every chat sandboxed, and this machine cannot apply the sandbox: socat \
         is not installed — install it with `sudo apt install socat` or `sudo dnf install \
         socat`. Nothing was started."
    );
}

#[test]
fn bwrap_is_installed_as_the_bubblewrap_package() {
    let missing = backend::missing(Os::Linux, &|_| false).expect("missing");
    assert_eq!(
        missing.to_string(),
        "bwrap and socat are not installed — install them with `sudo apt install bubblewrap socat` \
         or `sudo dnf install bubblewrap socat`"
    );
}

#[test]
fn on_windows_a_sandboxed_plane_starts_no_chat_until_a_backend_exists() {
    let plane = plane_saying(ON);
    let refused = for_start(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Windows),
        &|_| true,
    );
    assert_eq!(
        refused,
        Err(NotStarted::NoBackend(backend::Missing::NoBackend(
            Os::Windows
        )))
    );
}

fn plane_with_a_keyring_vault() -> tempfile::TempDir {
    let plane = plane_saying(ON);
    std::fs::write(
        plane.path().join("vaults.json"),
        r#"{"vaults": {"dev": {"provider": "keyring"}}}"#,
    )
    .expect("the registry");
    plane
}

#[test]
fn a_sandboxed_project_with_a_keyring_vault_starts_every_harness_on_macos() {
    // Ruling V90: the store holds Claude Code's, and charter's own wrap Codex's and opencode's.
    let plane = plane_with_a_keyring_vault();
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let applied = for_start(harness, plane.path(), &machine(Os::MacOs), &|_| true)
            .unwrap_or_else(|refused| panic!("{harness:?}: {refused}"));
        assert!(applied.is_some(), "{harness:?} starts sandboxed");
    }
}

#[test]
fn on_linux_a_keyring_vault_refusal_offers_the_opt_out_and_moving_the_vault() {
    let plane = plane_with_a_keyring_vault();
    let refused = for_start(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| true,
    )
    .expect_err("refused");
    let said = refused.to_string();
    assert_eq!(
        said,
        "this project runs every chat sandboxed, and on Linux purlis cannot keep a Claude \
         Code chat away from the system keyring, where this project's keyring vaults keep their \
         secrets, so nothing was started. Start this chat without the sandbox from the \
         new-chat picker, or move those secrets to a plain-file or 1Password vault, which the \
         sandbox can keep from a chat. For a resumed or relaunched chat, moving them is the \
         way on."
    );
    assert!(!said.contains("cannot keep a chat away from the operating system's credential store"));
}

#[test]
fn claude_code_is_denied_submodule_git_config_and_hook_managers_at_any_depth() {
    // Measured on 2.1.288: a `**` in the middle of a `denyWrite` glob holds at any depth.
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    let write = deny_write(&settings);
    for glob in [
        "**/.git/modules/**/config",
        "**/.git/modules/**/hooks",
        "**/.husky",
        "**/.githooks",
    ] {
        assert!(write.iter().any(|it| it == glob), "{glob} in {write:?}");
        assert!(settings.deny.contains(&format!("Edit({glob})")), "{glob}");
    }
}

#[test]
fn a_clone_the_app_made_for_a_chat_keeps_its_git_config_hooks_and_editor_settings_denied_to_it() {
    // #1335: the app writes a sandboxed chat's clone and worktree for it, and the chat's own
    // writes to what they carry stay refused — `git config` included, at any depth.
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    let write = deny_write(&settings);
    for glob in [
        "**/.git/config",
        "**/.git/hooks",
        "**/.git/worktrees",
        "**/.vscode",
        "**/.claude/settings.json",
        // What would point git at a config, objects or attributes of the chat's own (D-1335-7).
        "**/.git/commondir",
        "**/.git/objects/info/alternates",
        "**/.git/info/attributes",
        "**/.git/modules/**/commondir",
        "**/.git/modules/**/objects/info/alternates",
        "**/.git/modules/**/info/attributes",
    ] {
        assert!(write.iter().any(|it| it == glob), "{glob} in {write:?}");
        assert!(settings.deny.contains(&format!("Edit({glob})")), "{glob}");
    }
}

#[test]
fn a_chat_in_a_branch_folder_is_given_nothing_under_its_clones_git_directory() {
    // #1055, ruling V99h: a sandboxed chat in a linked worktree commits through the app, and
    // its sandbox is not widened for it. The worktree's git directory is in its clone's
    // `.git/worktrees`, which stays a later-code name on every harness, and the clone is
    // outside what a chat standing in the branch folder writes.
    for name in [
        ".git",
        ".git/worktrees",
        ".git/commondir",
        ".git/config.worktree",
    ] {
        assert!(PLANTED.iter().any(|planted| planted.path == name), "{name}");
    }
    let cwd = Path::new("/project/workspaces/w/.worktrees/repo/piece");
    let tmp = Path::new("/tmp-of/the-chat");
    let profile = seatbelt::profile(&[], &seatbelt::Own::default(), cwd, tmp, &[4040], None)
        .expect("a profile");
    // The wrap (Codex, opencode): the two folders it may write, and no other folder of the
    // project, the clone least of all.
    let subpaths: Vec<&str> = profile
        .lines()
        .skip_while(|line| *line != "(allow file-write*")
        .skip(1)
        .take_while(|line| line.trim_start().starts_with('('))
        .filter(|line| line.contains("subpath"))
        .collect();
    assert_eq!(
        subpaths,
        [
            "  (subpath \"/project/workspaces/w/.worktrees/repo/piece\")",
            "  (subpath \"/tmp-of/the-chat\")",
        ],
        "{profile}"
    );
    assert!(!profile.contains("/project/workspaces/w/repo"), "{profile}");
    // And the folder's own `.git` file is held there, so the link cannot be rewritten.
    let rules = seatbelt::planted_rules(cwd).expect("rules");
    assert!(
        rules.iter().any(|rule| rule.contains("\\\\.git$")),
        "{rules:?}"
    );
    // Claude Code: no write is added, and the worktree's git directory stays denied by name.
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    let filesystem = &settings.sandbox["filesystem"];
    assert!(
        deny_write(&settings)
            .iter()
            .any(|it| it == "**/.git/worktrees"),
        "{filesystem}"
    );
    assert!(
        !filesystem.to_string().contains("allowWrite")
            || !filesystem["allowWrite"].to_string().contains(".git"),
        "{filesystem}"
    );
}

#[test]
fn what_a_hooks_path_or_a_project_config_names_is_denied_to_every_harness_from_the_start() {
    // Ruling V73d: resolved when the chat starts, as path denials of the later-code class.
    let plane = plane_saying(ON);
    let clone = plane.path().join("workspaces/w/repo");
    std::fs::create_dir_all(clone.join(".git")).expect("a clone");
    std::fs::write(clone.join(".git/config"), "[core]\nhooksPath = hooks\n").expect("config");
    std::fs::create_dir_all(clone.join(".claude")).expect(".claude");
    std::fs::write(
        clone.join(".claude/settings.local.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"command": "./guard.sh"}]}]}}"#,
    )
    .expect("settings");
    let denied = Denied::of(plane.path(), &machine(Os::MacOs));
    for want in [clone.join("hooks"), clone.join("guard.sh")] {
        assert!(
            denied.paths.iter().any(|it| it.path == want
                && it.class == Class::LaterCode
                && it.access == Access::Write),
            "{want:?} not denied: {:?}",
            denied.paths
        );
    }
}

/// Whether `denied` holds `path` as later code a chat may not write.
fn denies_later_code(denied: &Denied, path: &std::path::Path) -> bool {
    denied
        .paths
        .iter()
        .any(|it| it.path == path && it.class == Class::LaterCode && it.access == Access::Write)
}

/// Gives the project at `plane` its own hook that runs `command`.
fn hook_runs(plane: &std::path::Path, command: &str) {
    std::fs::create_dir_all(plane.join(".claude")).expect(".claude");
    let settings = serde_json::json!({"hooks": {"PreToolUse": [{"hooks": [
        {"type": "command", "command": command}
    ]}]}});
    std::fs::write(
        plane.join(".claude/settings.local.json"),
        settings.to_string(),
    )
    .expect("settings");
}

#[test]
fn a_later_letter_s_value_in_a_folder_chats_may_be_granted_is_denied() {
    // D-T56-1: a folder you list as one chats may be granted (D-1342-10), outside the project
    // and the home folder, is one a chat could be given to write; `-xI<there>` names it.
    let plane = tempfile::tempdir().expect("a plane");
    hook_runs(plane.path(), "cc -xI/opt/granted/inc/x.h a.c");
    let want = std::path::Path::new("/opt/granted/inc/x.h");
    assert!(!denies_later_code(
        &Denied::of(plane.path(), &machine(Os::Linux)),
        want
    ));
    local::list_grantable(plane.path(), std::path::Path::new("/opt/granted")).expect("listed");
    let denied = Denied::of(plane.path(), &machine(Os::Linux));
    assert!(denies_later_code(&denied, want), "{:?}", denied.paths);
}

#[test]
fn a_later_letter_s_value_in_the_project_s_cache_home_is_denied() {
    // D-T56-1: the project's cache home (#1337) is under purlis's data home, which
    // `XDG_DATA_HOME` can move out of the home folder; a chat writes it.
    let machine = Machine {
        env: crate::secrets::Env::of(&[("XDG_DATA_HOME", "/srv/data")]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: Os::Linux,
    };
    let plane = tempfile::tempdir().expect("a plane");
    let cache = caches::root_of(&machine, plane.path()).expect("a data home");
    assert!(cache.starts_with("/srv/data"), "{cache:?}");
    // A cluster whose letters purlis does not know (gawk's own are read as gawk reads them,
    // D-1418-1).
    let want = cache.join("npm/x.h");
    hook_runs(plane.path(), &format!("cc -xI{} a.c", want.display()));
    assert!(!denies_later_code(
        &Denied::of(
            plane.path(),
            &Machine {
                env: crate::secrets::Env::of(&[]),
                ..machine.clone()
            }
        ),
        &want
    ));
    let denied = Denied::of(plane.path(), &machine);
    assert!(denies_later_code(&denied, &want), "{:?}", denied.paths);
}

#[test]
fn a_charter_toml_that_cannot_be_read_starts_no_chat_rather_than_one_unsandboxed() {
    // Absent `[sandbox]` is "not set", so a file charter cannot parse must not read as that.
    let plane = plane_saying("[sandbox\nmode = \"on\"\n");
    for harness in Harness::ALL {
        let refused =
            for_start(harness, plane.path(), &machine(Os::MacOs), &|_| true).expect_err("refused");
        assert_eq!(refused, NotStarted::PlaneUnreadable, "{harness:?}");
        assert_eq!(
            refused.to_string(),
            "charter.toml in this project cannot be read as TOML, so purlis cannot tell whether \
             it runs chats sandboxed, and nothing was started. Fix charter.toml and start the \
             chat again."
        );
    }
    // No file at all says nothing, so the chat would start at the floor (#2071); a start
    // refuses it first ([`decide`], D-1410e).
    let none = tempfile::tempdir().expect("a directory");
    let started =
        for_start(Harness::Codex, none.path(), &machine(Os::MacOs), &|_| true).expect("a start");
    assert_eq!(started.map(|applied| applied.level()), Some(Level::Floor));
}

#[test]
fn the_directories_between_a_root_and_a_path_are_each_named_once() {
    let root = std::path::Path::new("/p");
    assert_eq!(
        ancestors_within(std::path::Path::new("/p/.charter/app/spool"), root),
        [
            std::path::PathBuf::from("/p/.charter"),
            std::path::PathBuf::from("/p/.charter/app")
        ]
    );
    assert!(ancestors_within(std::path::Path::new("/p/x"), root).is_empty());
    assert!(ancestors_within(std::path::Path::new("/q/x/y"), root).is_empty());
}

/// What `for_start` answers in a plane whose `charter.toml` is `make`'s.
#[cfg(unix)]
fn started_with(make: impl Fn(&std::path::Path)) -> Result<Option<Applied>, NotStarted> {
    let plane = tempfile::tempdir().expect("a plane");
    make(&plane.path().join("charter.toml"));
    for_start(Harness::Codex, plane.path(), &machine(Os::MacOs), &|_| true)
}

/// What puts something at a path, for a test of what is there.
#[cfg(unix)]
type Maker = dyn Fn(&std::path::Path) + Send;

#[cfg(unix)]
#[test]
fn a_charter_toml_that_is_not_a_regular_file_starts_no_chat() {
    let sandboxed = |dir: &std::path::Path| {
        let target = dir.join("real.toml");
        std::fs::write(&target, ON).expect("a real file");
        target
    };
    // A link to a file that turns the sandbox on, a dangling link, a directory, a FIFO, a socket.
    let cases: Vec<(&str, Box<Maker>)> = vec![
        (
            "a link",
            Box::new(move |at: &std::path::Path| {
                let target = sandboxed(at.parent().expect("a parent"));
                std::os::unix::fs::symlink(target, at).expect("a link");
            }),
        ),
        (
            "a dangling link",
            Box::new(|at: &std::path::Path| {
                std::os::unix::fs::symlink(at.with_extension("gone"), at).expect("a link");
            }),
        ),
        (
            "a directory",
            Box::new(|at: &std::path::Path| std::fs::create_dir(at).expect("a directory")),
        ),
        (
            "a FIFO",
            Box::new(|at: &std::path::Path| {
                let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(at))
                    .expect("mkfifo runs");
                assert!(made.success(), "a FIFO");
            }),
        ),
        (
            "a socket",
            Box::new(|at: &std::path::Path| {
                std::mem::forget(std::os::unix::net::UnixListener::bind(at).expect("a socket"));
            }),
        ),
    ];
    for (what, make) in cases {
        let (sender, answer) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(started_with(make).map(|it| it.is_some()));
        });
        // A FIFO with no writer must not hold the start up.
        let started = answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{what}: the start blocked"));
        assert_eq!(started, Err(NotStarted::PlaneUnreadable), "{what}");
    }
}

#[cfg(unix)]
#[test]
fn a_device_is_not_read_as_a_plane_file() {
    // An endless device would otherwise be read until memory ran out.
    for device in ["/dev/zero", "/dev/null"] {
        assert_eq!(
            read_plane_file(std::path::Path::new(device)),
            Err(()),
            "{device}"
        );
    }
}

#[test]
fn a_charter_toml_over_the_cap_starts_no_chat() {
    let plane = tempfile::tempdir().expect("a plane");
    let mut text = String::from(ON);
    text.push_str(&"# padding\n".repeat(PLANE_FILE_MAX / 10 + 1));
    std::fs::write(plane.path().join("charter.toml"), &text).expect("charter.toml");
    assert_eq!(
        for_start(Harness::Codex, plane.path(), &machine(Os::MacOs), &|_| true),
        Err(NotStarted::PlaneUnreadable)
    );
    // Under the cap, the same words are read.
    std::fs::write(plane.path().join("charter.toml"), ON).expect("charter.toml");
    assert!(
        for_start(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::MacOs),
            &|_| true
        )
        .expect("starts")
        .is_some()
    );
}

#[cfg(unix)]
#[test]
fn no_sandboxed_chat_starts_in_a_folder_reached_through_a_link() {
    // Ruling of 2026-10-03, every harness: a plane-root chat could swap a workspace folder for
    // a link, and a chat started there would take that folder's rules to the link's target.
    let plane = plane_saying(ON_WITHOUT_CACHES);
    let workspaces = plane.path().join("workspaces");
    std::fs::create_dir_all(workspaces.join("real")).expect("a real workspace");
    let elsewhere = tempfile::tempdir().expect("somewhere outside");
    std::os::unix::fs::symlink(elsewhere.path(), workspaces.join("swapped")).expect("a link");
    std::fs::write(workspaces.join("a-file"), "").expect("a file");
    std::os::unix::fs::symlink(workspaces.join("real"), plane.path().join("ws-link"))
        .expect("a link to a real folder inside");
    let words = |program: &str| Words {
        program: program.to_owned(),
        command: Vec::new(),
        armed: Vec::new(),
        charters: Vec::new(),
    };
    for harness in Harness::ALL {
        let applied =
            compiled_anyway(harness, plane.path(), &machine(Os::MacOs)).expect("compiles");
        let confinement = applied.confine().expect("confined");
        let at = |cwd: std::path::PathBuf| (cwd, confinement.as_ref());
        for (cwd, want) in [
            (workspaces.join("swapped"), FolderRefusal::Linked),
            (workspaces.join("swapped/below"), FolderRefusal::Linked),
            (plane.path().join("ws-link"), FolderRefusal::Linked),
            (workspaces.join("a-file"), FolderRefusal::Linked),
            (workspaces.join("missing"), FolderRefusal::Linked),
            (elsewhere.path().to_path_buf(), FolderRefusal::Outside),
        ] {
            let (cwd, confinement) = at(cwd);
            assert_eq!(
                applied.line(
                    words("harness"),
                    &At {
                        cwd: Some(&cwd),
                        confinement,
                        ..At::default()
                    }
                ),
                Err(want.said(&policy::Locks::none())),
                "{harness:?} in {}",
                cwd.display()
            );
        }
        // A real folder inside the plane starts.
        assert!(
            applied
                .line(
                    words("harness"),
                    &At {
                        cwd: Some(&workspaces.join("real")),
                        confinement: confinement.as_ref(),
                        ..At::default()
                    }
                )
                .is_ok(),
            "{harness:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_charter_toml_above_a_folder_that_is_not_a_regular_file_is_seen() {
    // The plane's own walk takes only a regular file for a plane, so a start that asked it
    // alone would read "no plane" and start the chat unsandboxed. Under either name (RN-2a).
    for (name, kind) in ["charter.toml", "purlis.toml"]
        .into_iter()
        .flat_map(|name| ["link", "dangling", "directory", "fifo"].map(|kind| (name, kind)))
    {
        let plane = tempfile::tempdir().expect("a plane");
        let marker = plane.path().join(name);
        match kind {
            "link" => {
                std::fs::write(plane.path().join("real.toml"), ON).expect("a file");
                std::os::unix::fs::symlink(plane.path().join("real.toml"), &marker)
                    .expect("a link");
            }
            "dangling" => std::os::unix::fs::symlink("gone", &marker).expect("a link"),
            "directory" => std::fs::create_dir(&marker).expect("a directory"),
            _ => {
                let made =
                    crate::forklock::status(std::process::Command::new("mkfifo").arg(&marker))
                        .expect("mkfifo runs");
                assert!(made.success());
            }
        }
        let below = plane.path().join("workspaces/w");
        std::fs::create_dir_all(&below).expect("a workspace");
        assert!(marker_unreadable(&below), "{name}: {kind}");
    }
    let plain = plane_saying(ON);
    assert!(!marker_unreadable(plain.path()));
    let none = tempfile::tempdir().expect("no plane");
    assert!(!marker_unreadable(none.path()));
}

#[cfg(unix)]
#[test]
fn a_folder_is_found_inside_its_plane_whichever_side_names_it_through_a_link() {
    // The no-profile path takes the plane as the kernel names it and the folder as the chat
    // recorded it (`/tmp/…` against `/private/tmp/…` on macOS, or a linked parent anywhere).
    let base = tempfile::tempdir().expect("a base");
    let real_parent = base.path().join("real");
    std::fs::create_dir_all(real_parent.join("plane/workspaces/w")).expect("a plane");
    std::fs::write(real_parent.join("plane/charter.toml"), ON).expect("charter.toml");
    std::os::unix::fs::symlink(&real_parent, base.path().join("linked")).expect("a linked parent");
    let real = real_parent.join("plane").canonicalize().expect("real");
    let linked = base.path().join("linked/plane");
    for (root, cwd) in [
        (real.clone(), linked.join("workspaces/w")),
        (linked.clone(), real.join("workspaces/w")),
        (linked.clone(), linked.join("workspaces/w")),
        (real.clone(), real.join("workspaces/w")),
    ] {
        assert_eq!(
            folder_refusal(&root, &cwd),
            None,
            "{} in {}",
            cwd.display(),
            root.display()
        );
    }
    // A link inside the plane is still one, from either side.
    std::os::unix::fs::symlink(real.join("workspaces/w"), real.join("workspaces/again"))
        .expect("a link inside");
    for (root, cwd) in [
        (real.clone(), linked.join("workspaces/again")),
        (linked.clone(), real.join("workspaces/again")),
    ] {
        assert_eq!(
            folder_refusal(&root, &cwd),
            Some(FolderRefusal::Linked),
            "{}",
            cwd.display()
        );
    }
}

#[cfg(unix)]
#[test]
fn every_harness_is_compiled_against_the_plane_as_the_kernel_names_it() {
    // A plane reached through a link (`/tmp` on macOS, a linked parent anywhere): one spelling
    // for every harness, so no rule depends on how the caller wrote the root.
    let base = tempfile::tempdir().expect("a base");
    std::fs::create_dir_all(base.path().join("real/plane")).expect("a plane");
    std::fs::write(base.path().join("real/plane/charter.toml"), ON).expect("charter.toml");
    std::os::unix::fs::symlink(base.path().join("real"), base.path().join("tmp"))
        .expect("a linked parent");
    let linked = base.path().join("tmp/plane");
    let real = linked.canonicalize().expect("real");
    assert_ne!(linked, real);
    let applied = for_start(Harness::ClaudeCode, &linked, &machine(Os::MacOs), &|_| true)
        .expect("starts")
        .expect("sandboxed");
    assert_eq!(applied.root(), real.as_path());
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!("compiled for Claude Code");
    };
    let write = settings.sandbox["filesystem"]["denyWrite"].to_string();
    assert!(
        write.contains(&real.join(".charter/app").display().to_string()),
        "{write}"
    );
    assert!(!write.contains(&linked.display().to_string()), "{write}");
}

/// The Claude Code settings a chat in `cwd` of the sandboxed plane at `root` is armed with.
fn claude_settings_in(root: &Path, cwd: &Path) -> claude::Settings {
    let applied = for_start(Harness::ClaudeCode, root, &machine(Os::MacOs), &|_| true)
        .expect("starts")
        .expect("sandboxed");
    let Form::ClaudeCode(settings) = applied.form_in(Some(cwd)) else {
        panic!("compiled for Claude Code");
    };
    settings
}

/// Whether `settings` deny writing `path`, to Claude Code's sandbox and to its Edit tool.
fn claude_holds(settings: &claude::Settings, path: &Path) -> bool {
    let path = path.display().to_string();
    deny_write(settings).contains(&path)
        && settings.deny.contains(&format!("Edit(/{path})"))
        && settings.deny.contains(&format!("Edit(/{path}/**)"))
}

#[test]
fn a_claude_code_chat_writes_a_manifest_only_where_it_cannot_change_a_chats_sandbox() {
    // #1336: a chat in `ws/repo` is denied the manifests of the root, `ws` and its own folder,
    // and may write one below its folder or in a temp folder.
    let plane = plane_saying(ON);
    let root = plane.path().canonicalize().expect("real");
    let repo = root.join("ws/repo");
    std::fs::create_dir_all(repo.join("sub")).expect("a repo");
    let settings = claude_settings_in(&root, &repo);
    for name in MANIFEST_NAMES {
        for folder in [root.clone(), root.join("ws"), repo.clone()] {
            let path = folder.join(name);
            assert!(
                claude_holds(&settings, &path),
                "{} is writable",
                path.display()
            );
        }
        let fixture = repo.join("sub").join(name);
        assert!(!claude_holds(&settings, &fixture), "{}", fixture.display());
        // No rule names a manifest by name alone, nor outside the project.
        let rules = deny_write(&settings)
            .into_iter()
            .chain(settings.deny.iter().cloned())
            .filter(|rule| rule.contains(name))
            .collect::<Vec<_>>();
        assert!(
            rules
                .iter()
                .all(|rule| !rule.contains("**/") && rule.contains(&root.display().to_string())),
            "{rules:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_claude_code_chat_reached_through_a_link_is_denied_its_manifests_by_both_names() {
    let base = tempfile::tempdir().expect("a base");
    std::fs::create_dir_all(base.path().join("real/plane/ws/repo")).expect("a plane");
    std::fs::write(base.path().join("real/plane/charter.toml"), ON).expect("charter.toml");
    std::os::unix::fs::symlink(base.path().join("real"), base.path().join("linked"))
        .expect("a linked parent");
    let linked = base.path().join("linked/plane");
    let real = linked.canonicalize().expect("real");
    assert_ne!(linked, real);
    let settings = claude_settings_in(&linked, &linked.join("ws/repo"));
    for name in MANIFEST_NAMES {
        for folder in ["", "ws", "ws/repo"] {
            for spelled in [&real, &linked] {
                let path = spelled.join(folder).join(name);
                assert!(
                    claude_holds(&settings, &path),
                    "{} is writable",
                    path.display()
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn a_root_manifest_that_is_a_link_is_denied_to_claude_code_by_its_own_name_and_its_target() {
    // #1336: the link itself, so it is never removed and replaced, and what it points at.
    let plane = plane_saying(ON);
    let root = plane.path().canonicalize().expect("real");
    let dotfiles = tempfile::tempdir().expect("a dotfiles repository");
    let target = dotfiles.path().join("purlis.local.toml");
    std::fs::write(&target, "").expect("the operator's settings");
    std::os::unix::fs::symlink(&target, root.join("purlis.local.toml")).expect("a link");
    let settings = claude_settings_in(&root, &root);
    for path in [
        root.join("purlis.local.toml"),
        target.canonicalize().expect("real"),
    ] {
        assert!(
            claude_holds(&settings, &path),
            "{} is writable",
            path.display()
        );
    }
}

#[test]
fn a_manifest_planted_below_a_workspace_names_no_project_of_its_own() {
    // Why a manifest below the chat's folder is left writable: the walk re-roots a marker
    // inside a project's `workspaces/` to that project (`plane::outermost`).
    let plane = plane_saying(ON);
    let root = plane.path().canonicalize().expect("real");
    let sub = root.join("workspaces/w/sub");
    std::fs::create_dir_all(&sub).expect("a folder");
    std::fs::write(sub.join("purlis.toml"), "[sandbox]\n").expect("a planted manifest");
    assert_eq!(crate::plane::find_root(&sub).expect("a plane"), root);
}

#[cfg(unix)]
#[test]
fn a_manifest_of_the_chats_folders_linked_to_its_ground_refuses_the_chat() {
    // #1327's backstop over the manifests #1336 adds: a link a chat planted in its folder, to
    // that folder or to `/`, would leave a later chat there read-only, so it is refused.
    for target in ["sub", "/"] {
        let plane = plane_saying(ON);
        let root = plane.path().canonicalize().expect("real");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).expect("a folder");
        let target = if target == "/" {
            std::path::PathBuf::from("/")
        } else {
            sub.clone()
        };
        std::os::unix::fs::symlink(&target, sub.join("purlis.local.toml")).expect("a link");
        let applied = for_start(Harness::ClaudeCode, &root, &machine(Os::MacOs), &|_| true)
            .expect("starts")
            .expect("sandboxed");
        let line = applied.line(
            Words {
                program: "claude".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &At {
                cwd: Some(&sub),
                hook_socket: None,
                confinement: None,
                no_opt_out: false,
                purlis: None,
            },
        );
        let why = line.expect_err("refused");
        assert!(
            why.contains("purlis.local.toml") && why.contains("nothing was started"),
            "{}: {why}",
            target.display()
        );
    }
}

#[test]
fn whether_a_harness_ever_starts_sandboxed_here_is_its_compiler_its_hold_and_its_wrap() {
    assert_eq!(never_on(Harness::ClaudeCode, Os::MacOs), None);
    assert_eq!(never_on(Harness::Opencode, Os::MacOs), None);
    // #1040: purlis's own wrap on Linux.
    assert_eq!(never_on(Harness::Opencode, Os::Linux), None);
    assert_eq!(
        never_on(Harness::Opencode, Os::Other).as_deref(),
        Some("purlis can wrap it on macOS and Linux only, so far")
    );
    // #1123: charter wraps Codex as it wraps opencode.
    assert_eq!(never_on(Harness::Codex, Os::MacOs), None);
    assert_eq!(never_on(Harness::Codex, Os::Linux), None);
}

/// #1422: the window shows why a held-back harness never starts sandboxed, and a ticket number
/// is no part of a sentence the window shows (`docs/ui-copy.md`). The issue stays with the
/// adapter that holds it back.
#[test]
fn a_held_back_harness_is_said_without_the_issue_it_waits_for() {
    let why = never_with(compiler(Harness::Codex), Some(1150), Os::MacOs)
        .expect("a held-back harness never starts sandboxed");
    assert_eq!(why, "purlis cannot keep its chats inside the sandbox yet");
    let refused = NotStarted::HeldBack(Harness::Codex, 1150).to_string();
    assert!(
        !refused.contains('#') && !refused.contains("1150"),
        "{refused}"
    );
}

/// #1422: which preset lets a chat write the project's package caches is the core's to say,
/// and the window reads it from the preset rather than keeping a copy of its word.
#[test]
fn only_the_toolchains_preset_widens_the_package_caches() {
    assert_eq!(
        Preset::ALL
            .into_iter()
            .filter(|preset| preset.widens_caches())
            .collect::<Vec<_>>(),
        [Preset::Toolchains]
    );
}

// -------------------------------------------------------------------------------------
// The per-chat opt-out (ADR 0067 §7) and Windows (ruling V21 3, V78 b)
// -------------------------------------------------------------------------------------

fn off(reason: Option<&str>) -> OptOut {
    OptOut {
        reason: reason.map(str::to_owned),
    }
}

#[test]
fn a_person_can_start_one_chat_without_the_sandbox_and_every_class_is_lifted_for_it() {
    let plane = plane_saying(ON);
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        Some(&off(Some("the build needs the network"))),
        None,
    );
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: Some("the build needs the network".to_owned()),
        })))
    );
    assert_eq!(Lifted::CLASSES, Class::ALL);
}

#[test]
fn a_project_whose_manifest_has_gone_never_reads_as_the_sandbox_being_off() {
    // D-1410e: a project with no manifest at its root cannot say whether it runs chats
    // sandboxed, so the chat is refused; a person's opt-out still starts it, audited.
    let gone = tempfile::tempdir().unwrap();
    for os in [Os::MacOs, Os::Linux] {
        let refused = decide(
            Harness::ClaudeCode,
            gone.path(),
            &machine(os),
            &|_| true,
            None,
            None,
        );
        assert_eq!(refused, Err(NotStarted::PlaneMissing), "{os:?}");
    }
    assert!(
        NotStarted::PlaneMissing
            .to_string()
            .starts_with(&format!("This project's {FILE} is missing")),
        "{}",
        NotStarted::PlaneMissing
    );
    let started = decide(
        Harness::ClaudeCode,
        gone.path(),
        &machine(Os::MacOs),
        &|_| true,
        Some(&off(None)),
        None,
    );
    assert_eq!(
        started,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: None,
        })))
    );
}

#[test]
fn the_opt_out_is_what_lets_a_chat_start_where_the_sandbox_cannot_be_applied() {
    let plane = plane_saying(ON);
    let refused = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| false,
        None,
        None,
    );
    assert!(
        matches!(refused, Err(NotStarted::NoBackend(_))),
        "{refused:?}"
    );
    let started = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| false,
        Some(&off(None)),
        None,
    );
    assert_eq!(
        started,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: None,
        })))
    );
}

#[test]
fn a_project_that_has_not_turned_the_sandbox_on_opts_out_of_the_floor() {
    let plane = plane_saying("schema = 1\n");
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        Some(&off(Some("why not"))),
        None,
    );
    // #2071: the floor is on, so a person's opt-out lifts it, audited.
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: Some("why not".to_owned()),
        })))
    );
}

#[test]
fn without_an_opt_out_a_sandboxed_project_still_sandboxes_or_refuses() {
    let plane = plane_saying(ON);
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        None,
        None,
    );
    assert!(
        matches!(decided, Ok(Some(Decided::Sandboxed(ref applied))) if applied.harness() == Harness::ClaudeCode),
        "{decided:?}"
    );
}

#[test]
fn on_windows_a_chat_in_a_sandboxed_project_starts_at_the_opt_out_and_charter_is_the_actor() {
    let plane = plane_saying(ON);
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Windows),
        &|_| true,
        None,
        None,
    );
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::NoBackend(Os::Windows),
            reason: None,
        })))
    );
}

#[test]
fn a_system_with_no_backend_that_is_not_windows_still_fails_closed() {
    let plane = plane_saying(ON);
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Other),
        &|_| true,
        None,
        None,
    );
    assert_eq!(
        decided,
        Err(NotStarted::NoBackend(backend::Missing::NoBackend(
            Os::Other
        )))
    );
}

#[test]
fn a_typed_reason_is_kept_to_one_short_line() {
    let long = format!("first line\nsecond {}", "x".repeat(600));
    let plane = plane_saying(ON);
    let Ok(Some(Decided::Unsandboxed(lifted))) = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        Some(&off(Some(&long))),
        None,
    ) else {
        panic!("unsandboxed");
    };
    let reason = lifted.reason.expect("a reason");
    assert!(!reason.contains('\n'), "{reason}");
    assert!(
        reason.chars().count() <= OptOut::MOST_REASON_CHARS,
        "{reason}"
    );
    let blank = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        Some(&off(Some("   "))),
        None,
    );
    assert_eq!(
        blank,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: None,
        })))
    );
}

#[test]
fn an_unsandboxed_chat_says_so_on_its_tab_and_why() {
    assert_eq!(
        Lifted {
            by: By::Person,
            reason: None,
        }
        .notice(),
        "This chat runs without the sandbox: you turned it off for this chat only. A new or \
         resumed chat does not inherit it."
    );
    assert_eq!(
        Lifted {
            by: By::NoBackend(Os::Windows),
            reason: None,
        }
        .notice(),
        "This chat runs without the sandbox: purlis has no sandbox backend on Windows yet, so \
         every chat here starts without it until one exists."
    );
}

// -------------------------------------------------------------------------------------
// SD-30's install action (ruling V78 c): the distribution's own command, typed and never run
// -------------------------------------------------------------------------------------

#[test]
fn the_install_action_is_the_distributions_own_package_manager_command() {
    let missing = backend::missing(Os::Linux, &|_| false).expect("missing");
    let cases = [
        (
            "ID=ubuntu\nID_LIKE=debian\n",
            "sudo apt install bubblewrap socat",
        ),
        ("ID=debian\n", "sudo apt install bubblewrap socat"),
        ("ID=\"fedora\"\n", "sudo dnf install bubblewrap socat"),
        (
            "ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n",
            "sudo dnf install bubblewrap socat",
        ),
        ("ID=arch\n", "sudo pacman -S bubblewrap socat"),
        (
            "ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n",
            "sudo zypper install bubblewrap socat",
        ),
        ("ID=alpine\n", "sudo apk add bubblewrap socat"),
    ];
    for (os_release, command) in cases {
        assert_eq!(
            backend::install_command(&missing, os_release).as_deref(),
            Some(command),
            "{os_release}"
        );
    }
}

#[test]
fn only_what_is_missing_is_installed() {
    let missing = backend::missing(Os::Linux, &|program| program == "bwrap").expect("missing");
    assert_eq!(
        backend::install_command(&missing, "ID=debian\n").as_deref(),
        Some("sudo apt install socat")
    );
}

#[test]
fn a_distribution_charter_does_not_know_gets_no_action_rather_than_a_guess() {
    let missing = backend::missing(Os::Linux, &|_| false).expect("missing");
    assert_eq!(backend::install_command(&missing, "ID=nixos\n"), None);
    assert_eq!(backend::install_command(&missing, ""), None);
}

#[test]
fn a_system_with_no_backend_has_nothing_to_install() {
    let missing = backend::missing(Os::Windows, &|_| false).expect("missing");
    assert_eq!(backend::install_command(&missing, "ID=debian\n"), None);
    let missing = backend::missing(Os::MacOs, &|_| false).expect("missing");
    assert_eq!(backend::install_command(&missing, "ID=debian\n"), None);
}

// -------------------------------------------------------------------------------------
// What the new-chat picker says before anything starts, and what each start records
// -------------------------------------------------------------------------------------

#[test]
fn the_picker_says_nothing_of_a_sandbox_a_project_has_not_turned_on() {
    let plane = plane_saying("schema = 1\n");
    assert_eq!(
        ahead(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::Linux),
            &|_| false,
            "",
            &|_| Ok(()),
        ),
        Ahead::Off
    );
}

#[test]
fn the_picker_says_a_chat_will_be_sandboxed_where_it_can_be() {
    let plane = plane_saying(ON);
    assert_eq!(
        ahead(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::MacOs),
            &|_| true,
            "",
            &|_| Ok(()),
        ),
        Ahead::Sandboxed
    );
}

#[test]
fn the_picker_shows_the_refusal_and_the_distributions_install_command_before_the_start() {
    let plane = plane_saying(ON);
    let Ahead::Refused { why, install } = ahead(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|program| program == "bwrap",
        "ID=debian\n",
        &|_| Ok(()),
    ) else {
        panic!("refused");
    };
    assert!(why.contains("socat is not installed"), "{why}");
    assert_eq!(install.as_deref(), Some("sudo apt install socat"));
}

#[test]
fn a_refusal_nothing_can_be_installed_for_offers_no_install() {
    let plane = plane_saying(ON);
    std::fs::write(
        plane.path().join("vaults.json"),
        r#"{"vaults": {"dev": {"provider": "keyring"}}}"#,
    )
    .expect("the registry");
    let Ahead::Refused { install, .. } = ahead(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| true,
        "ID=debian\n",
        &|_| Ok(()),
    ) else {
        panic!("refused");
    };
    assert_eq!(install, None);
}

#[test]
fn on_windows_the_picker_says_the_chat_starts_without_the_sandbox_and_why() {
    let plane = plane_saying(ON);
    assert_eq!(
        ahead(
            Harness::Codex,
            plane.path(),
            &machine(Os::Windows),
            &|_| true,
            "",
            &|_| Ok(()),
        ),
        Ahead::Unsandboxed(Lifted {
            by: By::NoBackend(Os::Windows),
            reason: None,
        })
    );
}

fn person() -> Lifted {
    Lifted {
        by: By::Person,
        reason: None,
    }
}

#[test]
fn a_new_chat_a_person_starts_unsandboxed_is_audited_off_and_counted_as_an_opt_out() {
    assert_eq!(
        at_start(Some(&person()), false, false, true),
        (Some(Change::Off(person())), Some(local::Started::OptedOut))
    );
}

#[test]
fn a_windows_start_is_audited_and_counted_apart_from_a_choice() {
    let windows = Lifted {
        by: By::NoBackend(Os::Windows),
        reason: None,
    };
    assert_eq!(
        at_start(Some(&windows), false, false, true),
        (
            Some(Change::Off(windows.clone())),
            Some(local::Started::NoBackend)
        )
    );
}

#[test]
fn a_chat_that_ran_unsandboxed_and_starts_sandboxed_again_is_audited_back_on_and_not_counted() {
    assert_eq!(at_start(None, true, true, false), (Some(Change::On), None));
}

#[test]
fn a_sandboxed_new_chat_is_counted_and_has_nothing_to_audit() {
    assert_eq!(
        at_start(None, true, false, true),
        (None, Some(local::Started::Sandboxed))
    );
}

#[test]
fn a_relaunch_is_audited_but_never_counted_again() {
    assert_eq!(
        at_start(Some(&person()), false, false, false),
        (Some(Change::Off(person())), None)
    );
}

#[test]
fn a_chat_in_a_project_without_the_sandbox_is_neither_audited_nor_counted() {
    assert_eq!(at_start(None, false, false, true), (None, None));
    assert_eq!(at_start(None, false, true, true), (None, None));
}

#[test]
fn an_unreadable_charter_toml_still_refuses_rather_than_reading_as_off() {
    let plane = plane_saying("[sandbox\nmode = \"on\"\n");
    assert_eq!(
        decide(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::MacOs),
            &|_| true,
            None,
            None,
        ),
        Err(NotStarted::PlaneUnreadable)
    );
    assert!(
        matches!(
            ahead(
                Harness::ClaudeCode,
                plane.path(),
                &machine(Os::MacOs),
                &|_| true,
                "",
                &|_| Ok(()),
            ),
            Ahead::Refused { .. }
        ),
        "the picker shows the refusal"
    );
    // The opt-out sits inside that refusal, and is audited as every other one is.
    assert_eq!(
        decide(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::MacOs),
            &|_| true,
            Some(&off(None)),
            None,
        ),
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: None,
        })))
    );
}

/// #1123 lifts ruling V87f's hold-back where charter wraps Codex, and #1040 wraps it on Linux.
/// Where the kernel cannot (no Landlock 6), the picker shows that refusal before anything
/// starts, and the opt-out inside it still starts the chat, audited.
#[test]
fn the_picker_shows_codex_sandboxed_where_charter_wraps_it_and_the_opt_out_elsewhere() {
    let plane = plane_saying(ON);
    for os in [Os::MacOs, Os::Linux] {
        assert_eq!(
            ahead(
                Harness::Codex,
                plane.path(),
                &machine(os),
                &|_| true,
                "",
                &|_| Ok(()),
            ),
            Ahead::Sandboxed,
            "{os:?}"
        );
    }
    let no_landlock = |program: &str| program != backend::LANDLOCK;
    let Ahead::Refused { why, install } = ahead(
        Harness::Codex,
        plane.path(),
        &machine(Os::Linux),
        &no_landlock,
        "ID=debian\n",
        &|_| Ok(()),
    ) else {
        panic!("refused on a kernel without Landlock 6");
    };
    assert!(
        why.contains("Landlock version 6 (Linux 6.12 or later)"),
        "{why}"
    );
    assert!(!why.contains('#'), "{why}");
    assert_eq!(install, None, "nothing to install fixes it");
    assert_eq!(
        decide(
            Harness::Codex,
            plane.path(),
            &machine(Os::Linux),
            &no_landlock,
            Some(&off(None)),
            None,
        ),
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: None,
        })))
    );
}

/// Ruling V87g, before the start: whatever the start's program check refuses — a relative
/// program, one wherever the chat can write, one not answering as the harness — is shown as a
/// refusal with "Start without the sandbox", and asked only where the chat would be sandboxed.
#[test]
fn the_picker_shows_every_refusal_of_the_program_check() {
    let plane = plane_saying(ON);
    for refusal in [
        NotStarted::ProgramRelative,
        NotStarted::ProgramWritable(plane.path().join("bin/claude")),
        NotStarted::WordWritable(plane.path().join("bin/run.sh").display().to_string()),
        NotStarted::WordTooLong,
        NotStarted::NotTheHarness(
            Harness::ClaudeCode,
            "/usr/local/bin/claude".to_owned(),
            None,
        ),
    ] {
        let said = refusal.to_string();
        let check = |_: &Applied| Err(refusal.clone());
        assert_eq!(
            ahead(
                Harness::ClaudeCode,
                plane.path(),
                &machine(Os::MacOs),
                &|_| true,
                "",
                &check,
            ),
            Ahead::Refused {
                why: said,
                install: None,
            }
        );
    }
    let asked = std::sync::atomic::AtomicBool::new(false);
    let check = |_: &Applied| {
        asked.store(true, std::sync::atomic::Ordering::SeqCst);
        Err(NotStarted::ProgramRelative)
    };
    assert!(matches!(
        ahead(
            Harness::ClaudeCode,
            plane.path(),
            &machine(Os::Windows),
            &|_| true,
            "",
            &check
        ),
        Ahead::Unsandboxed(_)
    ));
    assert!(
        !asked.load(std::sync::atomic::Ordering::SeqCst),
        "not asked where the chat is not sandboxed"
    );
}

/// A file denial does not stop a connect to a unix socket: a sandbox treats that connect as
/// network. So no compiler may allow one beyond what it must, or a chat could reach
/// `charterd.sock` through a folder it cannot read (FD-27, ADR 0068 §5). What it must is the
/// chat's own hook socket, which a `purlis` command the chat runs asks the app over (ADR 0067
/// §2, #1328): compiled, Claude Code's sandbox allows no unix socket, and where the chat opens
/// it is allowed that one path, as the kernel names it, and never every socket. Codex and
/// opencode run in charter's wrap, which allows the hook socket alone (`codex_tests`,
/// `opencode_tests`).
/// Holds that `line`, a wrapped harness's, names none of the variables a `purlis` command in
/// the chat asks the app with (#1408), under any spelling: the socket, the chat's number, its
/// token, whether it runs sandboxed, and where the app started it. The app sets those as it
/// starts every chat, whatever its harness, and a line only adds to its environment, so a
/// command in a Codex or opencode chat hands its write over as one in a Claude Code chat does.
pub(super) fn names_no_hook_channel_variable(line: &Line) {
    let channel = [
        crate::hookwire::SOCKET_ENV,
        crate::hookwire::CHAT_ENV,
        crate::hookwire::TOKEN_ENV,
        crate::hookwire::SANDBOXED_ENV,
        crate::active::WORKSPACE_ENV,
    ];
    let named: Vec<&str> = line
        .env
        .iter()
        .map(|(key, _)| key.as_str())
        .filter(|key| channel.iter().any(|ours| crate::envvar::same(ours, key)))
        .collect();
    assert!(named.is_empty(), "the line sets {named:?}: {:?}", line.env);
}

#[test]
fn claude_code_allows_a_chat_no_unix_socket_but_its_hook_socket() {
    let (_plane, denied) = denied_with(None, Os::MacOs);
    let settings = claude::settings(&compiled(denied, Os::MacOs)).expect("compiles");
    let unix = |sandbox: &serde_json::Value| -> serde_json::Map<String, serde_json::Value> {
        sandbox["network"]
            .as_object()
            .expect("a network object")
            .iter()
            .filter(|(key, _)| key.to_lowercase().contains("unix"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    };
    // Every socket refused in so many words, so a user's `true` cannot merge in.
    assert_eq!(
        serde_json::Value::Object(unix(&settings.sandbox)),
        serde_json::json!({"allowAllUnixSockets": false})
    );
    assert_eq!(settings.reporting_on(None), settings);

    let sockets = tempfile::tempdir().expect("a directory");
    let sockets_at = sockets.path().canonicalize().expect("the directory");
    let socket = sockets_at.join("hooks.sock");
    let reporting = settings.reporting_on(Some(&socket));
    assert_eq!(
        serde_json::Value::Object(unix(&reporting.sandbox)),
        serde_json::json!({
            "allowAllUnixSockets": false,
            "allowUnixSockets": [real(&socket).display().to_string()],
        })
    );
    // Nothing else moves but the socket's folder, denied to writes, so the bind the grant
    // carries can never replace the app's socket: as the kernel names it, and on the data
    // volume's other side where it has one (#1418).
    let folders: Vec<String> = std::iter::once(sockets_at.clone())
        .chain(firmlink_twin(&sockets_at))
        .map(|it| it.display().to_string())
        .collect();
    let mut without = reporting.clone();
    without.sandbox["network"]
        .as_object_mut()
        .expect("a network object")
        .remove("allowUnixSockets");
    let denied = without.sandbox["filesystem"]["denyWrite"]
        .as_array_mut()
        .expect("a denyWrite list");
    let added = denied.split_off(denied.len() - folders.len());
    assert_eq!(
        added,
        folders
            .iter()
            .map(|it| serde_json::json!(it))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        without.deny.split_off(settings.deny.len()),
        folders
            .iter()
            .flat_map(|folder| [format!("Edit(/{folder})"), format!("Edit(/{folder}/**)")])
            .collect::<Vec<_>>()
    );
    assert_eq!(without, settings);
}

#[test]
fn charters_own_wrap_never_lets_a_chat_ask_the_keychains_service() {
    // Ruling V90b: the wrap Codex and opencode run in denies the credential store's service.
    // Seatbelt denies by default, so it is held by no lookup of it being allowed, anywhere.
    let dir = tempfile::tempdir().expect("a directory");
    let cwd = dir.path().join("chat");
    let tmp = dir.path().join("tmp");
    let profile = seatbelt::profile(&[], &seatbelt::Own::default(), &cwd, &tmp, &[4040], None)
        .expect("a profile");
    let probe = seatbelt::probe_profile(&tmp).expect("a profile");
    for text in [&profile, &probe] {
        assert!(text.starts_with("(version 1)\n(deny default)\n"), "{text}");
        for service in ["SecurityServer", "securityd", "com.apple.security"] {
            assert!(!text.contains(service), "{service} is reachable:\n{text}");
        }
        for line in text.lines().filter(|line| line.contains("mach-lookup")) {
            assert_eq!(line, "(allow mach-lookup", "a lookup not by name: {line}");
        }
    }
}

#[test]
fn a_chat_started_in_a_worktree_writes_that_folder_and_nothing_of_the_clone_it_was_cut_from() {
    // #1453: a dispatch may start its persona chat in a worktree the app cut for it. The start
    // is the one every chat in a branch folder has, and this is what it compiles to: the
    // worktree is the ground, and the clone it was cut from, that clone's `.git` and the
    // worktree's own git data there (`.git/worktrees/<piece>`) are not writable. That last one
    // is why such a chat's own `git commit` is refused and the app commits for it (#1055,
    // ruling V99h): the rule was kept, and a change to it fails here first.
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path()).expect("resolved");
    let clone = root.join("workspaces/alpha/api");
    let cwd = root.join("workspaces/alpha/.worktrees/api/check-the-queue-b5rc0def");
    let tmp = root.join("tmp");
    for made in [&clone, &cwd, &tmp] {
        std::fs::create_dir_all(made).expect("a folder");
    }

    let profile = seatbelt::profile(&[], &seatbelt::Own::default(), &cwd, &tmp, &[4040], None)
        .expect("a profile");

    // Every path the profile lets the chat write, by the rule that names it.
    let block = profile
        .split_once("(allow file-write*\n")
        .expect("what it may write")
        .1;
    let allowed: Vec<&str> = block
        .lines()
        .map(str::trim)
        .take_while(|line| *line != ")")
        .filter(|line| line.starts_with("(subpath ") || line.starts_with("(literal "))
        .collect();
    let named = |path: &std::path::Path| format!("\"{}\"", path.display());
    assert!(
        allowed.contains(&format!("(subpath {})", named(&cwd)).as_str()),
        "{allowed:?}"
    );
    assert!(
        allowed.contains(&format!("(subpath {})", named(&tmp)).as_str()),
        "{allowed:?}"
    );
    // Nothing above the worktree, and nothing of the clone.
    for not_its in [
        clone.clone(),
        clone.join(".git"),
        clone.join(".git/worktrees/check-the-queue-b5rc0def"),
        root.join("workspaces/alpha"),
        root.join("workspaces/alpha/.worktrees"),
        root.join("workspaces/alpha/.worktrees/api"),
        root.clone(),
    ] {
        let rule = named(&not_its);
        assert!(
            !allowed.iter().any(|line| line.contains(rule.as_str())),
            "{} is writable: {allowed:?}",
            not_its.display()
        );
    }
    let writes: Vec<&&str> = allowed
        .iter()
        .filter(|line| line.contains(root.to_string_lossy().as_ref()))
        .collect();
    assert_eq!(
        writes.len(),
        2,
        "its folder and its temp folder: {writes:?}"
    );
}

// -------------------------------------------------------------------------------------
// The ground a chat stands on is never denied (#1327)
// -------------------------------------------------------------------------------------

fn later_code(path: &str, file: &str, word: &str) -> Denial {
    Denial {
        named: Some(Named {
            file: std::path::PathBuf::from(file),
            word: word.to_owned(),
        }),
        ..one(Class::LaterCode, path, Access::Write)
    }
}

/// `/`, the home directory, and the chat's folder with every folder above it.
fn ground_of(cwd: &str) -> Vec<std::path::PathBuf> {
    let mut ground: Vec<_> = std::path::Path::new(cwd)
        .ancestors()
        .map(std::path::Path::to_path_buf)
        .collect();
    ground.push(std::path::PathBuf::from("/home/op"));
    ground
}

#[test]
fn a_denial_that_covers_the_chat_s_ground_refuses_it_naming_the_file_and_the_word() {
    let ground = ground_of("/plane/ws/repo");
    for path in [
        "/",
        "/home/op",
        "/plane",
        "/plane/ws",
        "/plane/ws/repo",
        "/plane/ws/repo/..",
        "/plane/ws/repo/nowhere/../..",
    ] {
        let denied = [later_code(path, "/plane/.claude/settings.json", "./")];
        assert_eq!(
            covering(&denied, &ground),
            Some(NotStarted::CoversItsGround {
                path: std::path::PathBuf::from(path),
                class: Class::LaterCode,
                named: Some(Named {
                    file: std::path::PathBuf::from("/plane/.claude/settings.json"),
                    word: "./".to_owned(),
                }),
                within: None,
            }),
            "{path}"
        );
    }
    assert_eq!(
        covering(
            &[later_code("/plane/ws", "/plane/ws/.mcp.json", "../ws")],
            &ground
        )
        .map(|it| it.to_string()),
        Some(
            "this project runs every chat sandboxed, and `../ws` in /plane/ws/.mcp.json reads as \
             a script purlis keeps this chat from changing, which would leave it unable to \
             write /plane/ws, so nothing was started. Change that word in /plane/ws/.mcp.json, \
             or start this chat without the sandbox from the new-chat picker."
                .to_owned()
        )
    );
    assert_eq!(
        covering(
            &[one(Class::Vaults, "/home/op", Access::ReadWrite)],
            &ground
        )
        .map(|it| it.to_string()),
        Some(
            "this project runs every chat sandboxed, and its vaults rules would keep the chat \
             from writing /home/op, so nothing was started. Start this chat without the \
             sandbox from the new-chat picker."
                .to_owned()
        )
    );
}

#[test]
fn a_denial_below_or_beside_the_chat_s_ground_is_kept() {
    let ground = ground_of("/plane/ws/repo");
    let denied = [
        later_code("/plane/ws/repo/scripts/x.sh", "/f", "./scripts/x.sh"),
        later_code("/plane/other", "/f", "../other"),
        later_code("/home/op/.config/x", "/f", "~/.config/x"),
        one(Class::Integrity, "/plane/.charter/app", Access::Write),
    ];
    assert_eq!(covering(&denied, &ground), None);
}

/// A plane with the sandbox on whose `dir` has a Claude Code hook running `command`.
fn plane_with_hook(dir: &str, command: &str) -> tempfile::TempDir {
    let plane = plane_saying(ON_WITHOUT_CACHES);
    let at = plane.path().join(dir);
    std::fs::create_dir_all(at.join(".claude")).expect(".claude");
    let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
        {"type": "command", "command": command}
    ]}]}});
    std::fs::write(at.join(".claude/settings.json"), settings.to_string()).expect("settings");
    plane
}

#[test]
fn what_a_wrap_adds_is_held_to_the_same_ground() {
    // A compiler's own denials, after the neutral ones: the operator's Codex home where
    // `CODEX_HOME` is the home directory.
    let compiled = Compiled {
        homes: Homes {
            home: Some(std::path::PathBuf::from("/home/op")),
            codex: Some(std::path::PathBuf::from("/home/op")),
            codex_project: Some(std::path::PathBuf::from("/data/codex-homes/x")),
            ..Homes::default()
        },
        ..compiled(Denied::default(), Os::MacOs)
    };
    let form = Form::Codex(codex::wrap(&compiled).expect("wraps"));
    assert_eq!(
        covering(
            form.denied().expect("a wrap's own list"),
            &ground_of("/plane/ws/repo")
        ),
        Some(NotStarted::CoversItsGround {
            path: std::path::PathBuf::from("/home/op"),
            class: Class::LaterCode,
            named: None,
            within: None,
        })
    );
}

#[test]
fn a_codex_home_that_is_the_home_directory_refuses_the_chat() {
    let plane = plane_saying(ON);
    let root = plane.path().canonicalize().expect("the plane");
    let machine = Machine {
        env: crate::secrets::Env::of(&[("CODEX_HOME", "/home/op")]),
        ..machine(Os::MacOs)
    };
    assert_eq!(
        for_start(Harness::Codex, &root, &machine, &|_| true),
        Err(NotStarted::CoversItsGround {
            path: std::path::PathBuf::from("/home/op"),
            class: Class::LaterCode,
            named: None,
            within: None,
        })
    );
}

#[test]
fn a_config_that_names_the_project_refuses_the_chat_rather_than_starting_it_read_only() {
    let plane = plane_saying(ON);
    let root = plane.path().canonicalize().expect("the plane");
    let clone = root.join("ws/repo");
    std::fs::create_dir_all(clone.join(".git")).expect("a clone");
    std::fs::write(clone.join(".git/config"), "[core]\nhooksPath = ../..\n").expect("config");
    // Every harness purlis sandboxes: the refusal is the neutral policy's, before any compiler.
    for harness in Harness::ALL
        .into_iter()
        .filter(|harness| never_on(*harness, Os::MacOs).is_none())
    {
        let started = for_start(harness, &root, &machine(Os::MacOs), &|_| true);
        let Err(NotStarted::CoversItsGround { path, named, .. }) = started else {
            panic!("{harness:?}: {started:?}");
        };
        assert_eq!(path, clone.join("../.."));
        assert_eq!(
            named,
            Some(Named {
                file: clone.join(".git/config"),
                word: "../..".to_owned(),
            })
        );
    }
}

#[test]
fn a_hook_word_that_names_the_chat_s_own_folder_refuses_a_chat_there_only() {
    let plane = plane_with_hook("ws/repo", "make -C ./tools/");
    let root = plane.path().canonicalize().expect("the plane");
    let tools = root.join("ws/repo/tools");
    std::fs::create_dir_all(&tools).expect("tools");
    let applied =
        compiled_anyway(Harness::ClaudeCode, &root, &machine(Os::MacOs)).expect("compiles");
    let words = Words {
        program: "claude".to_owned(),
        command: Vec::new(),
        armed: Vec::new(),
        charters: Vec::new(),
    };
    let line = |cwd: &std::path::Path| {
        applied.line(
            words.clone(),
            &At {
                cwd: Some(cwd),
                ..At::default()
            },
        )
    };
    assert_eq!(
        line(&tools),
        Err(NotStarted::CoversItsGround {
            path: tools.clone(),
            class: Class::LaterCode,
            named: Some(Named {
                file: root.join("ws/repo/.claude/settings.json"),
                word: "./tools/".to_owned(),
            }),
            within: None,
        }
        .to_string())
    );
    assert!(line(&root.join("ws/repo")).is_ok());
}

#[test]
fn a_jq_hook_leaves_the_chat_s_folder_and_its_workspace_writable() {
    // #1327, as seen: the hook below made every chat in the project read-only.
    let plane = plane_with_hook(
        "workspaces/w/repo",
        "jq -r '.tool_response.filePath // .tool_input.file_path' | xargs -r prettier --write",
    );
    let root = plane.path().canonicalize().expect("the plane");
    let workspace = root.join("workspaces/w");
    let clone = workspace.join("repo");
    let applied =
        compiled_anyway(Harness::ClaudeCode, &root, &machine(Os::MacOs)).expect("compiles");
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!("a Claude Code form");
    };
    // Whether a rule, `/`-rooted or not, holds `path`: a `**/<name>` rule names a name at any
    // depth and holds no folder by itself; any other is the folder it names and what is below.
    let holds = |rule: &str, path: &std::path::Path| {
        if rule.starts_with("**/") {
            return false;
        }
        let folder = rule.trim_end_matches("**").trim_end_matches('/');
        folder.is_empty() || path.starts_with(folder)
    };
    let written = |path: &std::path::Path| {
        let by_sandbox = deny_write(settings).iter().any(|it| holds(it, path));
        let by_tools = settings.deny.iter().any(|rule| {
            rule.strip_prefix("Edit(")
                .and_then(|it| it.strip_suffix(')'))
                .is_some_and(|it| holds(it.strip_prefix('/').unwrap_or(it), path))
        });
        !by_sandbox && !by_tools
    };
    // The helper holds what #1327 wrote.
    assert!(holds("/", &root) && holds("/**", &root) && holds("**", &root));
    assert!(holds(&format!("{}/**", workspace.display()), &clone));
    for folder in [&clone, &workspace, &root] {
        assert!(
            written(folder),
            "{} is denied: {:?} {:?}",
            folder.display(),
            deny_write(settings),
            settings.deny
        );
    }
    assert!(!deny_write(settings).iter().any(|it| it == "/"));
    assert!(
        !settings
            .deny
            .iter()
            .any(|it| it == "Edit(//)" || it == "Edit(//**)")
    );
    let words = Words {
        program: "claude".to_owned(),
        command: Vec::new(),
        armed: Vec::new(),
        charters: Vec::new(),
    };
    assert!(
        applied
            .line(
                words,
                &At {
                    cwd: Some(&clone),
                    ..At::default()
                }
            )
            .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn a_denial_through_a_link_and_a_missing_folder_is_still_held_to_the_ground() {
    let base = tempfile::tempdir().expect("a base");
    let real_dir = base.path().join("real");
    std::fs::create_dir_all(real_dir.join("x")).expect("a real folder");
    std::os::unix::fs::symlink(&real_dir, base.path().join("link")).expect("a link");
    let denied = [later_code(
        base.path()
            .join("link/missing/..")
            .to_str()
            .expect("a path"),
        "/f",
        "../..",
    )];
    let ground = [real_dir.join("x").canonicalize().expect("the folder")];
    assert!(covering(&denied, &ground).is_some());
}

#[test]
fn a_config_purlis_could_not_read_through_refuses_the_chat() {
    let named = Named {
        file: std::path::PathBuf::from("/plane/ws/.claude/settings.json"),
        word: "./d64".to_owned(),
    };
    let compiled = compiled(
        Denied {
            unread: Some(named.clone()),
            ..Denied::default()
        },
        Os::MacOs,
    );
    let compile = compiler(Harness::ClaudeCode).expect("a compiler");
    let refused = compile_checked(
        compile,
        &compiled,
        std::path::Path::new("/plane"),
        &machine(Os::MacOs),
    )
    .map(|_| ())
    .expect_err("refused");
    assert_eq!(refused, NotStarted::Unread(named));
    assert_eq!(
        refused.to_string(),
        "this project runs every chat sandboxed, and a command in \
         /plane/ws/.claude/settings.json changes folder or names scripts more often than purlis \
         follows, from `./d64` on, so purlis cannot tell what it runs, and nothing was started. \
         Move that command into a script of its own, or start this chat without the sandbox \
         from the new-chat picker."
    );
}

/// [`compile_checked`] as it was before #2570: the second [`covering`] pass run whatever the
/// harness, also where it adds no denial of its own and so checks the very list the first did.
fn compile_checked_both_passes(
    compile: Compiler,
    compiled: &Compiled,
    root: &std::path::Path,
    machine: &Machine,
) -> Result<(Form, Vec<Denial>), NotStarted> {
    if let Some(unread) = &compiled.denied.unread {
        return Err(NotStarted::Unread(unread.clone()));
    }
    if let Some(why) = &compiled.denied.untold {
        return Err(NotStarted::KeptUntold(why.clone()));
    }
    let ground = ground(root, machine.home.as_deref());
    if let Some(refused) = covering(&compiled.denied.paths, &ground) {
        return Err(refused.placed_in(root));
    }
    let form = compile(compiled).map_err(NotStarted::Uncompilable)?;
    let denied = form
        .denied()
        .map_or_else(|| compiled.denied.paths.clone(), <[Denial]>::to_vec);
    if let Some(refused) = covering(&denied, &ground) {
        return Err(refused.placed_in(root));
    }
    Ok((form, denied))
}

/// The bytes a Claude Code chat's `--settings` gets from `form`.
fn claude_bytes(form: &Form) -> String {
    let Form::ClaudeCode(settings) = form else {
        panic!("a Claude Code form, got {form:?}");
    };
    format!(
        "{}\n{}",
        serde_json::to_string(&settings.sandbox).expect("the sandbox object"),
        serde_json::to_string(&settings.deny).expect("the deny rules")
    )
}

/// #2570: where the harness adds no denial of its own (Claude Code), the second covering pass
/// is skipped, and the profile it compiles is byte for byte the one both passes gave, as is
/// every refusal.
#[test]
fn skipping_the_second_covering_pass_for_claude_code_changes_no_byte_of_its_profile() {
    let compile = compiler(Harness::ClaudeCode).expect("a compiler");
    let root = std::path::Path::new("/plane");
    let machine = machine(Os::MacOs);
    let denials = vec![
        one(Class::Vaults, "/home/op/.ssh", Access::ReadWrite),
        one(Class::Vaults, "/plane/.purlis/vaults", Access::ReadWrite),
        later_code("/plane/ws/.git/hooks", "/plane/ws/.git/config", "hooksPath"),
        one(Class::HumanPowers, "/elsewhere/keys", Access::Write),
    ];
    let plane = compiled(
        Denied {
            paths: denials.clone(),
            ..Denied::default()
        },
        Os::MacOs,
    );
    assert_eq!(compile(&plane).expect("compiles").denied(), None);

    let skipped = compile_checked(compile, &plane, root, &machine).expect("compiles");
    let both = compile_checked_both_passes(compile, &plane, root, &machine).expect("compiles");
    assert_eq!(skipped, both);
    assert_eq!(claude_bytes(&skipped.0), claude_bytes(&both.0));
    assert_eq!(skipped.1, denials);

    // A denial that covers the ground is refused the same way, by the first pass.
    let covers = compiled(
        Denied {
            paths: [
                denials,
                vec![one(Class::HumanPowers, "/home", Access::Write)],
            ]
            .concat(),
            ..Denied::default()
        },
        Os::MacOs,
    );
    let refused = compile_checked(compile, &covers, root, &machine).expect_err("refused");
    assert_eq!(
        Err(refused),
        compile_checked_both_passes(compile, &covers, root, &machine)
    );
}

/// #1333's rule, which every brokered write asks (ADR 0067 §1 as amended): a chat never changes
/// the sandbox, at any level, through purlisd.
#[test]
fn a_brokered_write_that_changes_any_sandbox_key_is_told_it_does() {
    let on = "schema = 1\n[sandbox]\nmode = \"on\"\nhosts = [\"a.example\"]\n";
    // Another key, and a comment, change no sandbox key.
    assert!(!changes_a_sandbox_key(
        Some(on),
        &format!("{on}\n[memory]\nshare = \"local\"\n# a note\n")
    ));
    assert!(!changes_a_sandbox_key(None, "schema = 1\n"));
    for after in [
        "schema = 1\n[sandbox]\nmode = \"on\"\nhosts = [\"a.example\", \"10.0.0.5\"]\n",
        "schema = 1\n[sandbox]\nmode = \"on\"\n",
        "schema = 1\n[sandbox]\nmode = \"on\"\nhosts = [\"a.example\"]\negress = []\n",
        "schema = 1\n",
        "not toml [",
    ] {
        assert!(changes_a_sandbox_key(Some(on), after), "{after}");
    }
    // This machine's file, made where there was none, with hosts of its own.
    assert!(changes_a_sandbox_key(
        None,
        "[sandbox]\nhosts = [\"10.0.0.5\"]\n"
    ));
    // A file that was not TOML is unknown, so a write over it is never let through as no change.
    assert!(changes_a_sandbox_key(Some("not toml ["), "schema = 1\n"));
}

/// Review of #1341, 8: a `[[forge]]` host is taken as a project's own host is, so it never
/// reaches this machine or a metadata service.
#[test]
fn a_forge_host_is_taken_as_a_host_is() {
    for host in ["127.0.0.1", "localhost", "169.254.169.254", "0x7f.1"] {
        let hosts = hosts_of(
            &[Preset::Forge],
            Some(&format!(
                "[[forge]]\nkind = \"gitlab\"\nhost = \"{host}\"\n"
            )),
        );
        assert!(!hosts.contains(&host.to_owned()), "{host}: {hosts:?}");
    }
}

/// #1405: a `[[forge]]` host the sandbox does not let through is said, in the reader's voice,
/// while the sandbox and the `forge` preset are on; never otherwise.
#[test]
fn a_forge_host_the_sandbox_drops_is_said() {
    let forge = "[[forge]]\nkind = \"gitlab\"\nhost = \"169.254.169.254\"\n\n\
                 [[forge]]\nkind = \"gitlab\"\nhost = \"git.example.org:8443\"\n";
    let on = said(&format!("[sandbox]\nmode = \"on\"\n{forge}"));
    let forged: Vec<String> = on
        .refused
        .iter()
        .filter(|refusal| matches!(refusal, Refusal::ForgeHost(..)))
        .map(ToString::to_string)
        .collect();
    assert_eq!(forged.len(), 1, "{:?}", on.refused);
    assert!(
        forged[0].starts_with(
            "forge.host in charter.toml names 169.254.169.254, which the sandbox does not let \
             chats reach: "
        ),
        "{}",
        forged[0]
    );
    // The host stays out of what chats reach, as before.
    assert!(
        !Plane::of(Some(&format!("[sandbox]\nmode = \"on\"\n{forge}")))
            .granted_hosts(&policy::Locks::none())
            .contains(&"169.254.169.254".to_owned())
    );
    // Not while the sandbox is off, nor while the project turns the forge preset off.
    for quiet in [
        forge.to_owned(),
        format!("[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n{forge}"),
    ] {
        assert!(
            !said(&quiet)
                .refused
                .iter()
                .any(|refusal| matches!(refusal, Refusal::ForgeHost(..))),
            "{quiet}"
        );
    }
    // A Settings save says it too.
    assert!(
        refusals(&format!("[sandbox]\nmode = \"on\"\n{forge}"), FILE)
            .iter()
            .any(|why| why.contains("169.254.169.254")),
    );
}

/// Review of #1341, 8: the Notice names a project's `[[forge]]` hosts with its own, while the
/// `forge` preset lets every chat reach them.
#[test]
fn the_hosts_a_teammate_is_told_of_include_the_forge_hosts() {
    let plane = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\nhosts = [\"10.0.0.5:6443\"]\n\n[[forge]]\nkind = \"gitlab\"\n\
         host = \"git.example.org:8443\"\n",
    ));
    assert_eq!(
        plane.granted_hosts(&policy::Locks::none()),
        ["10.0.0.5:6443", "git.example.org"]
    );
    let without = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\negress = []\n\n[[forge]]\nkind = \"gitlab\"\n\
         host = \"git.example.org\"\n",
    ));
    assert_eq!(
        without.granted_hosts(&policy::Locks::none()),
        Vec::<String>::new()
    );
}

/// Review of #1341, round 3: a chat can write all of a clone's git state, so this machine's
/// file grants only what you confirmed in Settings. A host a chat (or a teammate's commit) put
/// there grants nothing until it is confirmed.
#[test]
fn this_machine_s_hosts_grant_only_what_you_confirmed() {
    let plane = plane_saying("[sandbox]\nmode = \"on\"\negress = []\n");
    std::fs::write(
        plane.path().join("charter.local.toml"),
        "[sandbox]\nhosts = [\"10.0.0.6\", \"10.0.0.7\"]\n",
    )
    .expect("written by a chat");
    assert_eq!(hosts::personal(plane.path()), Vec::<hosts::Host>::new());
    local::confirm_host(plane.path(), "10.0.0.7").expect("confirmed");
    assert_eq!(
        hosts::personal(plane.path()),
        [hosts::Host::parse("10.0.0.7").unwrap()]
    );
}

/// Review of #1341, 6: this machine's file is never read through a link.
#[cfg(unix)]
#[test]
fn this_machine_s_hosts_through_a_link_grant_nothing() {
    let plane = plane_saying("[sandbox]\nmode = \"on\"\negress = []\n");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let target = elsewhere.path().join("hosts.toml");
    std::fs::write(&target, "[sandbox]\nhosts = [\"10.0.0.6\"]\n").expect("target");
    std::os::unix::fs::symlink(&target, plane.path().join("charter.local.toml")).expect("link");
    assert_eq!(hosts::personal(plane.path()), Vec::<hosts::Host>::new());
}

// -------------------------------------------------------------------------------------
// What a project's sandbox widens past its hosts (spec #1330, #1337): the project's own
// package caches with the toolchains preset, the certificate check only where the project
// turns it on
// -------------------------------------------------------------------------------------

use std::path::{Path as StdPath, PathBuf as StdPathBuf};

/// A Mac whose home is `/Users/op`, with `env`, and a terminal's `PATH` unless `env` says one.
fn mac_with(env: &[(&str, &str)]) -> Machine {
    let mut vars = vec![(
        "PATH",
        "/Users/op/.cargo/bin:/Users/op/go/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
    )];
    vars.retain(|(key, _)| !env.iter().any(|(set, _)| set == key));
    vars.extend_from_slice(env);
    Machine {
        env: crate::secrets::Env::of(&vars),
        home: Some(StdPathBuf::from("/Users/op")),
        os: Os::MacOs,
    }
}

const PROJECT: &str = "/Users/op/project";

fn policy_of(presets: &[Preset], certificate_checks: bool) -> Policy {
    Policy {
        egress: presets.to_vec(),
        hosts: vec![],
        certificate_checks,
        personas: Default::default(),
    }
}

/// What `harness` starts under in the project at `root` with `policy`, on `machine`: compiled
/// as a start compiles it, with no `charter.toml` written.
fn compiled_for(harness: Harness, policy: &Policy, root: &StdPath, machine: &Machine) -> Applied {
    applied_for(harness, policy, &Plane::of(None), root, machine).expect("compiles")
}

fn under_presets(harness: Harness, presets: &[Preset], machine: &Machine) -> Applied {
    compiled_for(
        harness,
        &policy_of(presets, false),
        StdPath::new(PROJECT),
        machine,
    )
}

/// The project at `root`'s cache home on `machine`, where its sandboxed chats' caches live.
fn cache_home(machine: &Machine, root: &StdPath) -> StdPathBuf {
    Homes::charter_data(machine)
        .expect("a data home")
        .join("cache-homes")
        .join(Homes::project_key(root))
}

fn claude_sandbox(applied: &Applied) -> &serde_json::Value {
    match applied.form() {
        Form::ClaudeCode(settings) => &settings.sandbox,
        other => panic!("not Claude Code's: {other:?}"),
    }
}

/// The Seatbelt profile `applied` wraps a chat in, for a wrapped harness.
fn wrap_profile(applied: &Applied) -> String {
    let cwd = StdPath::new(PROJECT);
    let tmp = StdPath::new("/Users/op/project-tmp");
    match applied.form() {
        Form::Opencode(wrap) => opencode::profile(wrap, cwd, tmp, &[4040], None),
        Form::Codex(wrap) => codex::profile(wrap, cwd, tmp, &[4040], None),
        Form::ClaudeCode(_) => panic!("Claude Code is not wrapped"),
    }
    .expect("a profile")
}

/// Every folder a chat writes whole in a cache home, in the order they are granted.
const TREES: [&str; 11] = [
    "cargo/registry",
    "cargo/git/checkouts",
    "npm",
    "pip",
    "go-mod",
    "go-build",
    "gradle",
    "yarn",
    "yarn-berry",
    "pnpm-store",
    "pnpm-cache",
];

/// The files a chat writes at the top of the project's cargo home.
const CARGO_FILES: [&str; 6] = [
    ".package-cache",
    ".package-cache-mutate",
    ".global-cache",
    ".global-cache-wal",
    ".global-cache-shm",
    ".global-cache-journal",
];

#[test]
fn the_toolchains_preset_gives_a_chat_its_projects_own_caches_and_only_with_it() {
    let machine = mac_with(&[]);
    let home = cache_home(&machine, StdPath::new(PROJECT));
    let on = under_presets(Harness::ClaudeCode, &[Preset::Toolchains], &machine);
    let mut expected: Vec<String> = TREES
        .iter()
        .map(|tree| home.join(tree).display().to_string())
        .collect();
    expected.extend(
        CARGO_FILES
            .iter()
            .map(|file| home.join("cargo").join(file).display().to_string()),
    );
    expected.push(format!("{}/cargo/git/db/*/**", home.display()));
    assert_eq!(
        claude_sandbox(&on)["filesystem"]["allowWrite"],
        serde_json::json!(expected)
    );
    // Every folder a chat may write is the project's own; none of the person's caches is.
    let writable = on.writable();
    assert_eq!(writable.len(), TREES.len() + 1 + CARGO_FILES.len());
    for path in &writable {
        assert!(
            path.starts_with(&home),
            "{} is not the project's",
            path.display()
        );
    }
    for person in [
        "/Users/op/.cargo",
        "/Users/op/.npm",
        "/Users/op/Library/Caches",
        "/Users/op/.cache",
        "/Users/op/go",
        "/Users/op/.gradle",
        "/Users/op/.yarn",
        "/Users/op/Library/pnpm",
    ] {
        assert!(
            !writable.iter().any(|path| path.starts_with(person)),
            "{person} is written"
        );
    }

    let off = under_presets(
        Harness::ClaudeCode,
        &[Preset::ModelProviders, Preset::Forge],
        &machine,
    );
    assert_eq!(claude_sandbox(&off)["filesystem"].get("allowWrite"), None);
    assert_eq!(off.writable(), Vec::<StdPathBuf>::new());

    for harness in [Harness::Opencode, Harness::Codex] {
        let on = wrap_profile(&under_presets(harness, &[Preset::Toolchains], &machine));
        let off = wrap_profile(&under_presets(harness, &[Preset::Forge], &machine));
        for tree in TREES {
            let tree = home.join(tree);
            let grant = format!("  (subpath \"{}\")\n", tree.display());
            assert!(
                on.contains(&grant),
                "{harness:?} does not grant {tree:?}:\n{on}"
            );
            // Pinned, so it is never removed, renamed or swapped for a link.
            let pin = format!("(deny file-write* (literal \"{}\"))\n", tree.display());
            assert!(on.contains(&pin), "{harness:?} does not pin {tree:?}");
            assert!(
                on.find(&pin) > on.find(&grant),
                "{harness:?}: pinned before granted"
            );
            // The later-code names stay denied at any depth inside each.
            let regex = format!(
                "^{}/(.*/)?\\.git/config(/.*)?$",
                seatbelt::escaped(&tree.display().to_string())
            );
            let planted = format!(
                "(deny file-write* file-link (regex {}))",
                seatbelt::string(&regex).expect("a string")
            );
            assert!(on.contains(&planted), "{harness:?}, {tree:?}:\n{on}");
        }
        for file in CARGO_FILES {
            let grant = format!(
                "  (literal \"{}\")\n",
                home.join("cargo").join(file).display()
            );
            assert!(on.contains(&grant), "{harness:?} does not grant {file}");
        }
        assert!(!off.contains("cache-homes"), "{harness:?}:\n{off}");
    }
}

#[test]
fn a_cargo_bare_repository_is_written_inside_and_never_made_moved_or_run() {
    let machine = mac_with(&[]);
    let db = cache_home(&machine, StdPath::new(PROJECT)).join("cargo/git/db");
    let on = under_presets(Harness::ClaudeCode, &[Preset::Toolchains], &machine);
    let sandbox = claude_sandbox(&on);
    let allowed = sandbox["filesystem"]["allowWrite"]
        .as_array()
        .expect("a list");
    assert!(!allowed.contains(&serde_json::json!(db.display().to_string())));
    let denied = sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a list");
    for glob in [
        format!("{}/*/config", db.display()),
        format!("{}/*/hooks", db.display()),
    ] {
        assert!(
            denied.contains(&serde_json::json!(glob)),
            "{glob} is not denied"
        );
    }
    // A `.git` at each folder's top, which pins the folder in Claude Code's profile.
    for tree in TREES {
        let pin = format!(
            "{}/.git",
            db.parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(tree)
                .display()
        );
        assert!(
            denied.contains(&serde_json::json!(pin)),
            "{pin} is not denied"
        );
    }
    for harness in [Harness::Opencode, Harness::Codex] {
        let profile = wrap_profile(&under_presets(harness, &[Preset::Toolchains], &machine));
        let escaped = seatbelt::escaped(&db.display().to_string());
        let inside = format!(
            "  (regex {})\n",
            seatbelt::string(&format!("^{escaped}/[^/]+/.+$")).expect("a string")
        );
        assert!(profile.contains(&inside), "{harness:?}:\n{profile}");
        let pin = format!("(deny file-write* (literal \"{}\"))", db.display());
        let run = format!(
            "(deny file-write* file-link (regex {}))",
            seatbelt::string(&format!("^{escaped}/[^/]+/(config|hooks)(/.*)?$")).expect("a string")
        );
        for rule in [&pin, &run] {
            assert!(
                profile.find(rule.as_str()) > profile.find(&inside),
                "{harness:?}: {rule}"
            );
        }
    }
}

#[test]
fn each_tool_is_pointed_at_its_projects_cache_and_the_folders_are_made_before_the_chat() {
    let base = tempfile::tempdir().expect("a base");
    let project = base.path().join("project");
    std::fs::create_dir_all(&project).expect("a project");
    let project = project.canonicalize().expect("the project");
    let data = base.path().join("data");
    let data_text = data.display().to_string();
    let machine = mac_with(&[(crate::datahome::HOME_VAR, data_text.as_str())]);
    let applied = compiled_for(
        Harness::ClaudeCode,
        &policy_of(&[Preset::Toolchains], false),
        &project,
        &machine,
    );
    let line = applied
        .line(
            Words {
                program: "claude".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &At {
                cwd: Some(&project),
                ..At::default()
            },
        )
        .expect("starts");
    let home = cache_home(&machine, &project);
    for (var, dir) in [
        ("CARGO_HOME", "cargo"),
        ("npm_config_cache", "npm"),
        ("NPM_CONFIG_CACHE", "npm"),
        ("PIP_CACHE_DIR", "pip"),
        ("GOMODCACHE", "go-mod"),
        ("GOCACHE", "go-build"),
        ("GRADLE_USER_HOME", "gradle"),
        ("YARN_CACHE_FOLDER", "yarn"),
        ("YARN_GLOBAL_FOLDER", "yarn-berry"),
        ("pnpm_config_store_dir", "pnpm-store"),
        ("pnpm_config_cache_dir", "pnpm-cache"),
        (concat!("npm", "_config_store_dir"), "pnpm-store"),
        (concat!("npm", "_config_cache_dir"), "pnpm-cache"),
    ] {
        let value = home.join(dir).display().to_string();
        assert!(
            line.env.contains(&(var.to_owned(), value.clone())),
            "{var}={value} is not in {:?}",
            line.env
        );
        assert!(home.join(dir).is_dir(), "{dir} was not made");
    }
    assert!(home.join("cargo/git/db").is_dir());
    let config = std::fs::read_to_string(home.join("cargo/config.toml")).expect("a config");
    assert!(config.starts_with("# Written by purlis"), "{config}");
}

#[cfg(unix)]
#[test]
fn a_link_planted_in_the_cache_home_is_taken_out_unfollowed_and_the_chat_starts() {
    let base = tempfile::tempdir().expect("a base");
    let project = base.path().join("project");
    std::fs::create_dir_all(&project).expect("a project");
    let project = project.canonicalize().expect("the project");
    let elsewhere = base.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");
    std::fs::write(elsewhere.join("config.toml"), "kept").expect("a file elsewhere");
    let data = base.path().join("data");
    let data_text = data.display().to_string();
    let machine = mac_with(&[(crate::datahome::HOME_VAR, data_text.as_str())]);
    let home = cache_home(&machine, &project);
    let policy = policy_of(&[Preset::Toolchains], false);
    let words = || Words {
        program: "claude".to_owned(),
        command: Vec::new(),
        armed: Vec::new(),
        charters: Vec::new(),
    };
    let at = At {
        cwd: Some(&project),
        ..At::default()
    };

    // A link planted where a cache folder goes, before the sandbox is compiled: taken out, and
    // the grant names the folder purlis makes, never where the link pointed.
    std::fs::create_dir_all(&home).expect("a cache home");
    std::os::unix::fs::symlink(&elsewhere, home.join("npm")).expect("a link");
    let applied = compiled_for(Harness::ClaudeCode, &policy, &project, &machine);
    assert!(
        std::fs::symlink_metadata(home.join("npm")).is_err(),
        "the link stayed"
    );
    let writable = applied.writable();
    assert!(writable.contains(&home.join("npm")), "{writable:?}");
    assert!(
        !writable.iter().any(|path| path.starts_with(&elsewhere)),
        "{writable:?}"
    );

    // Links planted after it was compiled, at a folder and at cargo's config: taken out at the
    // start, nothing written through them, and the chat starts.
    std::fs::create_dir_all(home.join("cargo/git")).expect("cargo's git");
    std::os::unix::fs::symlink(&elsewhere, home.join("cargo/git/db")).expect("a link");
    std::os::unix::fs::symlink(
        elsewhere.join("config.toml"),
        home.join("cargo/config.toml"),
    )
    .expect("a link");
    applied.line(words(), &at).expect("starts");
    for folder in ["npm", "cargo/git/db"] {
        let meta = std::fs::symlink_metadata(home.join(folder)).expect("made");
        assert!(meta.is_dir() && !meta.file_type().is_symlink(), "{folder}");
    }
    let config = std::fs::symlink_metadata(home.join("cargo/config.toml")).expect("written");
    assert!(config.is_file() && !config.file_type().is_symlink());
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("config.toml")).expect("read"),
        "kept"
    );
    assert_eq!(std::fs::read_dir(&elsewhere).expect("read").count(), 1);
}

#[test]
fn a_cache_link_purlis_cannot_take_out_is_named_in_the_refusal() {
    let path = std::path::Path::new("/data/cache-homes/k/npm");
    assert_eq!(
        caches_linked(path, "after"),
        "this project runs every chat sandboxed, and /data/cache-homes/k/npm of the project's \
         package caches is a link purlis could not take out (after making them), so nothing \
         was started. Remove that link and start the chat again."
    );
}

#[test]
fn a_projects_cargo_home_keeps_where_crates_come_from_and_nothing_else() {
    let person = r#"
[registries.corp]
index = "sparse+https://crates.corp.example/index/"
token = "a-secret"
credential-provider = "cargo:token-from-stdout /usr/local/bin/creds"

[registry]
default = "corp"
token = "another-secret"
global-credential-providers = ["/usr/local/bin/creds"]

[source.crates-io]
replace-with = "vendored"

[source.vendored]
directory = "vendor"

[build]
rustc-wrapper = "/usr/local/bin/sccache"

[target.x86_64-apple-darwin]
runner = "/usr/local/bin/run"

[alias]
b = "build"
"#;
    let seeded = cargo_config_text(person);
    let parsed: toml::Table = seeded.parse().expect("TOML");
    let expected: toml::Table = r#"
[registries.corp]
index = "sparse+https://crates.corp.example/index/"

[registry]
default = "corp"

[source.crates-io]
replace-with = "vendored"
"#
    .parse()
    .expect("TOML");
    assert_eq!(parsed, expected);
    for word in ["secret", "creds", "sccache", "runner", "alias"] {
        assert!(!seeded.contains(word), "{word} is in:\n{seeded}");
    }
    // Nothing purlis can read is nothing.
    assert_eq!(
        cargo_config_text("not toml [")
            .parse::<toml::Table>()
            .expect("TOML"),
        toml::Table::new()
    );
}

fn cargo_config_text(person: &str) -> String {
    caches::cargo_config(person, None)
}

/// A person's cargo home at `<base>/cargo` whose config is `config`, its relative paths
/// resolved as cargo resolves them: against the folder above the one the config is in (#1364).
fn cargo_config_in(base: &std::path::Path, config: &str) -> toml::Table {
    let home = base.join("cargo");
    std::fs::create_dir_all(&home).expect("a cargo home");
    caches::cargo_config(config, Some(base))
        .parse()
        .expect("TOML")
}

fn source_of<'a>(seeded: &'a toml::Table, name: &str) -> Option<&'a toml::Value> {
    seeded.get("source").and_then(|source| source.get(name))
}

#[test]
fn a_relative_source_path_is_resolved_where_cargo_resolves_it() {
    let base = tempfile::tempdir().expect("a folder");
    std::fs::create_dir_all(base.path().join("vendor")).expect("vendor");
    std::fs::create_dir_all(base.path().join("regs/local")).expect("a local registry");
    let seeded = cargo_config_in(
        base.path(),
        "[source.crates-io]\nreplace-with = \"vendored\"\n\
         [source.vendored]\ndirectory = \"vendor\"\n\
         [source.mine]\nlocal-registry = \"./regs/local\"\n",
    );
    let canonical = |rel: &str| {
        base.path()
            .join(rel)
            .canonicalize()
            .expect("there")
            .display()
            .to_string()
    };
    assert_eq!(
        source_of(&seeded, "vendored")
            .and_then(|entry| entry.get("directory"))
            .and_then(toml::Value::as_str),
        Some(canonical("vendor").as_str())
    );
    assert_eq!(
        source_of(&seeded, "mine")
            .and_then(|entry| entry.get("local-registry"))
            .and_then(toml::Value::as_str),
        Some(canonical("regs/local").as_str())
    );
    // The replacement that names it is untouched.
    assert_eq!(
        source_of(&seeded, "crates-io")
            .and_then(|entry| entry.get("replace-with"))
            .and_then(toml::Value::as_str),
        Some("vendored")
    );
}

#[test]
fn a_relative_source_path_that_climbs_out_or_follows_a_link_out_is_dropped() {
    let outer = tempfile::tempdir().expect("a folder");
    let base = outer.path().join("base");
    std::fs::create_dir_all(base.join("inside")).expect("inside");
    std::fs::create_dir_all(outer.path().join("outside")).expect("outside");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outer.path().join("outside"), base.join("link")).expect("a link");
    let seeded = cargo_config_in(
        &base,
        "[source.crates-io]\nreplace-with = \"up\"\n\
         [source.up]\ndirectory = \"../outside\"\n\
         [source.round]\ndirectory = \"inside/../inside\"\n\
         [source.linked]\ndirectory = \"link\"\n\
         [source.through]\nlocal-registry = \"link/.\"\n\
         [source.gone]\ndirectory = \"not-there\"\n\
         [source.empty]\ndirectory = \"\"\n",
    );
    for name in ["up", "round", "linked", "through", "gone", "empty"] {
        assert_eq!(source_of(&seeded, name), None, "{name} was kept: {seeded}");
    }
    // A replacement naming a dropped entry stays, so cargo says the source is missing rather
    // than quietly fetching from elsewhere.
    assert_eq!(
        source_of(&seeded, "crates-io")
            .and_then(|entry| entry.get("replace-with"))
            .and_then(toml::Value::as_str),
        Some("up")
    );
}

#[test]
fn a_relative_source_path_with_no_folder_to_resolve_against_is_dropped() {
    let seeded: toml::Table = caches::cargo_config(
        "[source.vendored]\ndirectory = \"vendor\"\n[source.abs]\ndirectory = \"/opt/vendor\"\n",
        None,
    )
    .parse()
    .expect("TOML");
    assert_eq!(source_of(&seeded, "vendored"), None);
    // An absolute path is copied as written, as before: the sandbox decides what a chat reads.
    assert_eq!(
        source_of(&seeded, "abs")
            .and_then(|entry| entry.get("directory"))
            .and_then(toml::Value::as_str),
        Some("/opt/vendor")
    );
}

/// #1364: of cargo's `[http]` and `[net]`, the project's cargo home keeps the certificate file
/// (data, not a program), the revocation check, fetching git with the git program, and
/// offline. Never the person's proxy: a sandboxed chat reaches the network through purlis's
/// own, which cargo finds in `HTTPS_PROXY`, and the file says the proxy was left out.
#[test]
fn a_projects_cargo_home_keeps_the_network_settings_that_route_nothing() {
    let seeded = cargo_config_text(
        "[http]\nproxy = \"http://proxy.corp.example:8080\"\ncainfo = \"/etc/corp/ca.pem\"\n\
         check-revoke = false\nssl-version = \"tlsv1.3\"\ndebug = true\n\
         [net]\ngit-fetch-with-cli = true\noffline = false\nretry = 5\n",
    );
    let parsed: toml::Table = seeded.parse().expect("TOML");
    let expected: toml::Table = "[http]\ncainfo = \"/etc/corp/ca.pem\"\ncheck-revoke = false\n\
         [net]\ngit-fetch-with-cli = true\noffline = false\n"
        .parse()
        .expect("TOML");
    assert_eq!(parsed, expected);
    assert!(!seeded.contains("corp.example"), "{seeded}");
    assert!(seeded.contains("# Left out: your [http] proxy"), "{seeded}");
    // A key of the wrong type is left out, and nothing to keep is no table.
    let typed: toml::Table = cargo_config_text("[http]\ncheck-revoke = \"no\"\ncainfo = 3\n")
        .parse()
        .expect("TOML");
    assert_eq!(typed, toml::Table::new());
    assert!(!cargo_config_text("").contains("Left out"));
}

/// A relative `cainfo` is resolved where cargo resolves it, as a `[source]` folder is, and
/// kept only when it is a file inside that folder.
#[test]
fn a_relative_cainfo_is_resolved_where_cargo_resolves_it_or_dropped() {
    let outer = tempfile::tempdir().expect("a folder");
    let base = outer.path().join("base");
    std::fs::create_dir_all(base.join("certs")).expect("certs");
    std::fs::write(base.join("certs/ca.pem"), "a certificate").expect("a certificate");
    std::fs::write(outer.path().join("outside.pem"), "outside").expect("outside");
    let cainfo = |written: &str| {
        cargo_config_in(&base, &format!("[http]\ncainfo = \"{written}\"\n"))
            .get("http")
            .and_then(|http| http.get("cainfo"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    assert_eq!(
        cainfo("certs/ca.pem"),
        Some(
            base.join("certs/ca.pem")
                .canonicalize()
                .expect("there")
                .display()
                .to_string()
        )
    );
    for dropped in ["../outside.pem", "certs", "certs/none.pem", ""] {
        assert_eq!(cainfo(dropped), None, "{dropped} was kept");
    }
}

/// The operator's ruling R1 (2026-10-13, amending ADR 0093): certificate checks are on for
/// every sandboxed chat unless the project's file says `false`.
#[test]
fn certificate_checks_are_on_unless_the_project_turns_them_off() {
    assert!(Policy::defaults().certificate_checks);
    assert_eq!(
        said("[sandbox]\nmode = \"on\"\n")
            .policy
            .map(|policy| policy.certificate_checks),
        Some(true),
        "an absent key reads as on"
    );
    let on = said("[sandbox]\nmode = \"on\"\ncertificate-checks = true\n");
    assert_eq!(
        on.policy.map(|policy| policy.certificate_checks),
        Some(true)
    );
    assert!(on.refused.is_empty(), "{:?}", on.refused);
    let off = said("[sandbox]\nmode = \"on\"\ncertificate-checks = false\n");
    assert_eq!(
        off.policy.map(|policy| policy.certificate_checks),
        Some(false)
    );
    assert!(off.refused.is_empty(), "{:?}", off.refused);
    // What cannot be read is refused with a sentence and read as off: a committed file only
    // ever narrows, so a value purlis cannot tell is the narrow one.
    let typo = said("[sandbox]\nmode = \"on\"\ncertificate-checks = \"yes\"\n");
    assert_eq!(
        typo.policy.map(|policy| policy.certificate_checks),
        Some(false)
    );
    assert_eq!(typo.refused, [Refusal::CertificateChecksNotABool]);
    assert_eq!(
        typo.refused[0].to_string(),
        "sandbox.certificate-checks in charter.toml is not true or false; certificate checks \
         are off until it is"
    );
}

/// R1, per harness: a default project's chat has the certificate check's one service in the
/// sandbox it starts in (Claude Code's own switch; the one `mach-lookup` rule in the profile
/// purlis wraps Codex and opencode in), `false` leaves it out, and so does an administrator's
/// `socket-grants` lock, whatever the project's file says.
#[test]
fn every_harness_starts_with_the_certificate_check_unless_the_project_or_policy_says_no() {
    let machine = mac_with(&[]);
    let locked = policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        StdPath::new("/etc/purlis/policy.json"),
    );
    for (policy, locks, on) in [
        // A default project, and one on every preset.
        (Policy::defaults(), policy::Locks::none(), true),
        (policy_of(&Preset::ALL, true), policy::Locks::none(), true),
        (policy_of(&[], true), policy::Locks::none(), true),
        // The project's own `certificate-checks = false`.
        (policy_of(&Preset::ALL, false), policy::Locks::none(), false),
        // An administrator's lock, over a project that leaves it on.
        (Policy::defaults(), locked.clone(), false),
        (policy_of(&Preset::ALL, true), locked, false),
    ] {
        policy::set_for_this_test(locks.clone());
        let applied = compiled_for(
            Harness::ClaudeCode,
            &policy,
            StdPath::new(PROJECT),
            &machine,
        );
        // Never left out: a user's `true` would merge in.
        assert_eq!(
            claude_sandbox(&applied)["enableWeakerNetworkIsolation"],
            serde_json::json!(on),
            "{policy:?} under {locks:?}"
        );
        for harness in [Harness::Opencode, Harness::Codex] {
            let profile = wrap_profile(&compiled_for(
                harness,
                &policy,
                StdPath::new(PROJECT),
                &machine,
            ));
            assert_eq!(
                profile.contains("(allow mach-lookup (global-name \"com.apple.trustd.agent\"))\n"),
                on,
                "{harness:?}, {policy:?} under {locks:?}:\n{profile}"
            );
            for service in ["SecurityServer", "securityd", "com.apple.security"] {
                assert!(!profile.contains(service), "{service}:\n{profile}");
            }
        }
        policy::set_for_this_test(policy::Locks::none());
    }
}

#[test]
fn the_ground_refusal_names_the_repo_and_the_workspace_its_config_sits_in() {
    // #1356: a word in a clone's config, then in a workspace's own, then in the project's.
    for (dir, place) in [
        (
            "workspaces/w/repo",
            Some("(in the repo workspaces/w/repo of workspace w)"),
        ),
        ("workspaces/w", Some("(in workspace w)")),
        ("ws/repo", Some("(in the repo ws/repo)")),
        ("", None),
    ] {
        let plane = plane_with_hook(dir, "make -C ./tools/");
        let root = plane.path().canonicalize().expect("the plane");
        let at = root.join(dir);
        if dir.ends_with("repo") {
            std::fs::create_dir_all(at.join(".git")).expect("a clone");
        }
        let tools = at.join("tools");
        std::fs::create_dir_all(&tools).expect("tools");
        let applied =
            compiled_anyway(Harness::ClaudeCode, &root, &machine(Os::MacOs)).expect("compiles");
        let words = Words {
            program: "claude".to_owned(),
            command: Vec::new(),
            armed: Vec::new(),
            charters: Vec::new(),
        };
        let refused = applied
            .line(
                words,
                &At {
                    cwd: Some(&tools),
                    ..At::default()
                },
            )
            .expect_err("refused");
        let file = at.join(".claude/settings.json");
        match place {
            Some(place) => assert!(
                refused.contains(&format!("in {} {place} reads", file.display())),
                "{dir}: {refused}"
            ),
            None => assert!(
                refused.contains(&format!("in {} reads", file.display())),
                "{dir}: {refused}"
            ),
        }
    }
}

#[cfg(unix)]
#[test]
fn a_missing_folder_that_differs_from_the_ground_only_in_case_covers_it_where_case_folds() {
    // #1356: on a volume that folds case, `NEW` and `new` are one folder before either exists.
    let base = tempfile::tempdir().expect("a base");
    let base = base.path().canonicalize().expect("the base");
    std::fs::create_dir_all(base.join("plane")).expect("the plane");
    let folds = std::fs::metadata(base.join("PLANE")).is_ok();
    let ground = [base.join("plane/new/repo")];
    for written in ["plane/NEW", "PLANE/new/repo", "Plane/New/Repo/x/.."] {
        let denied = [later_code(
            base.join(written).to_str().expect("a path"),
            "/f",
            written,
        )];
        assert_eq!(covering(&denied, &ground).is_some(), folds, "{written}");
    }
    // Beside the ground in any case is still beside it.
    let denied = [later_code(
        base.join("PLANE/newer").to_str().expect("a path"),
        "/f",
        "../newer",
    )];
    assert_eq!(covering(&denied, &ground), None);
}

#[cfg(target_os = "macos")]
#[test]
fn a_denial_in_the_data_volume_spelling_is_held_to_the_ground_and_both_ways() {
    // #1356: macOS reaches a firmlinked folder (`/Users`, `/private`, …) by two names, and
    // resolving either leaves its spelling as it is.
    let base = tempfile::tempdir().expect("a base");
    let base = base.path().canonicalize().expect("the base");
    std::fs::create_dir_all(base.join("plane/ws")).expect("the plane");
    let data = std::path::Path::new("/System/Volumes/Data")
        .join(base.strip_prefix("/").expect("absolute"));
    assert!(data.join("plane").is_dir(), "{}", data.display());
    let plain = base.join("plane");
    let spelled = data.join("plane");
    for (denial, ground) in [(&spelled, &plain), (&plain, &spelled)] {
        let denied = [later_code(denial.to_str().expect("a path"), "/f", "x")];
        assert!(
            covering(&denied, &[ground.join("ws")]).is_some(),
            "{} over {}",
            denial.display(),
            ground.display()
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_denied_path_is_denied_under_its_data_volume_spelling_too() {
    // #1356: as a path through a link is denied by both names (FD-27).
    for (written, twin) in [
        (
            "/Users/op/tools/x.sh",
            "/System/Volumes/Data/Users/op/tools/x.sh",
        ),
        ("/System/Volumes/Data/Users/op/y.sh", "/Users/op/y.sh"),
    ] {
        let denied = Denied {
            paths: vec![later_code(written, "/f", "x")],
            ..Denied::default()
        };
        let settings = claude::settings(&compiled(denied, Os::MacOs)).expect("compiles");
        for name in [written, twin] {
            assert!(deny_write(&settings).iter().any(|it| it == name), "{name}");
            assert!(
                settings.deny.contains(&format!("Edit(/{name})")),
                "{name}: {:?}",
                settings.deny
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn no_directory_a_chat_searches_is_one_its_sandbox_lets_it_write() {
    // A directory on a chat's PATH that a chat can write is a program the next chat, the app's
    // own forge calls and the operator's terminal run without a sandbox. What a chat may write
    // is read from the compiled sandbox itself, for every harness with a compiler and every
    // preset on, so a grant added later is checked here without anyone listing it.
    const FINDER: &str = "/usr/bin:/bin:/usr/sbin:/sbin";
    let home = StdPathBuf::from("/Users/op");
    for (machine, grants) in [
        (mac_with(&[]), true),
        (mac_with(&[("PATH", FINDER)]), true),
        (
            mac_with(&[
                ("CARGO_HOME", "/Users/op/tools/cargo"),
                ("GOBIN", "/Users/op/gobin"),
            ]),
            true,
        ),
        // A data home on the chat's PATH, or holding cargo's home: no cache home there.
        (
            mac_with(&[(crate::datahome::HOME_VAR, "/Users/op/.local/bin")]),
            false,
        ),
        // A data home that is the home itself still keeps the caches in a folder of their own.
        (mac_with(&[(crate::datahome::HOME_VAR, "/Users/op")]), true),
        (
            mac_with(&[(crate::datahome::HOME_VAR, "/Users/op/.cargo")]),
            false,
        ),
        (
            mac_with(&[(crate::datahome::HOME_VAR, "/Users/op/project/.data")]),
            false,
        ),
    ] {
        let path = machine.env.get("PATH").expect("a PATH");
        let mut searched: Vec<StdPathBuf> = std::env::split_paths(
            &crate::programs::chat_path_from(Some(std::ffi::OsStr::new(&path)), Some(&home), None)
                .expect("a chat's PATH"),
        )
        .collect();
        searched.extend(crate::programs::search_dirs_from(
            Some(std::ffi::OsStr::new(&path)),
            Some(&home),
        ));
        // And where a later program is run from whether or not this PATH names it: cargo's
        // home and its `bin` (its config names programs cargo runs), Go's `bin`, pnpm's home.
        searched.extend(caches::never_covered(&machine));
        searched.extend(
            [
                "/Users/op/.cargo",
                "/Users/op/.cargo/bin",
                "/Users/op/.cargo/config.toml",
                "/Users/op/.cargo/credentials.toml",
                "/Users/op/go/bin",
                "/Users/op/Library/pnpm",
            ]
            .map(StdPathBuf::from),
        );
        for harness in [Harness::ClaudeCode, Harness::Opencode, Harness::Codex] {
            let applied = under_presets(harness, &Preset::ALL, &machine);
            let caches = applied.caches.as_ref().map(|caches| caches.writable());
            assert_eq!(caches.is_some(), grants, "{harness:?}: {caches:?}");
            let mut writable = applied.writable();
            writable.extend(program::temp_roots(&[]));
            writable.push(StdPathBuf::from(PROJECT));
            for dir in &searched {
                for grant in &writable {
                    assert!(
                        !dir.starts_with(grant),
                        "{harness:?}: {} is searched, and a chat may write {}",
                        dir.display(),
                        grant.display()
                    );
                }
            }
        }
    }
}

/// What macOS tools read in place of `TMPDIR` (#1120, measured on macOS 26.2), each pointed into
/// the wrapped chat's own temp directory: `TMP` and `TEMP`; `xcrun`'s cache, which it otherwise
/// writes in the system's per-user temp folder; and clang's module cache, which clang and Swift
/// otherwise write in the per-user cache folder and fail without.
#[test]
fn a_wrapped_chat_points_what_tools_read_in_place_of_tmpdir_at_its_own_temp_directory() {
    let env: std::collections::BTreeMap<String, String> =
        seatbelt::env("http://127.0.0.1:1", Path::new("/c/.purlis-tmp/x"))
            .into_iter()
            .collect();
    for (key, value) in [
        ("TMPDIR", "/c/.purlis-tmp/x"),
        ("TMP", "/c/.purlis-tmp/x"),
        ("TEMP", "/c/.purlis-tmp/x"),
        ("xcrun_db", "/c/.purlis-tmp/x/xcrun_db"),
        (
            "CLANG_MODULE_CACHE_PATH",
            "/c/.purlis-tmp/x/clang-module-cache",
        ),
    ] {
        assert_eq!(env.get(key).map(String::as_str), Some(value), "{key}");
    }
}

#[test]
fn a_config_that_names_the_project_is_named_with_its_repo_and_workspace() {
    // #1356, before any chat's folder: the refusal at the start says where the config sits.
    let plane = tempfile::tempdir().expect("a plane");
    let root = plane.path().canonicalize().expect("the plane");
    std::fs::create_dir_all(root.join("workspaces/w/repo/.git")).expect("a clone");
    std::fs::create_dir_all(root.join("workspaces/v/.claude")).expect("a workspace");
    let compile = compiler(Harness::ClaudeCode).expect("a compiler");
    for (file, place) in [
        (
            "workspaces/w/repo/.claude/settings.json",
            " (in the repo workspaces/w/repo of workspace w) ",
        ),
        ("workspaces/v/.claude/settings.json", " (in workspace v) "),
        (".claude/settings.json", " "),
    ] {
        let file = root.join(file);
        let denied = Denied {
            paths: vec![later_code(
                root.to_str().expect("a path"),
                file.to_str().expect("a path"),
                "../..",
            )],
            ..Denied::default()
        };
        let refused = compile_checked(
            compile,
            &compiled(denied, Os::MacOs),
            &root,
            &machine(Os::MacOs),
        )
        .map(|_| ())
        .expect_err("refused")
        .to_string();
        assert!(
            refused.contains(&format!("in {}{place}reads as a script", file.display())),
            "{refused}"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn charters_own_wrap_denies_a_path_and_pins_its_folders_under_both_firmlink_names() {
    // #1356 review F1: the kernel may name a firmlinked folder either way, so a deny rule and
    // the pin on each folder between the chat's and the denied path carry both.
    let cwd = std::path::Path::new("/Users/op-1356/plane");
    let tmp = std::path::Path::new("/Users/op-1356/tmp");
    for written in [
        "/Users/op-1356/plane/tools/hook.sh",
        "/System/Volumes/Data/Users/op-1356/plane/tools/hook.sh",
    ] {
        let denied = [later_code(written, "/f", "./tools/hook.sh")];
        let profile =
            seatbelt::profile(&denied, &seatbelt::Own::default(), cwd, tmp, &[4040], None)
                .expect("a profile");
        let rules: Vec<&str> = profile
            .lines()
            .filter(|line| {
                line.starts_with("(deny file-write* file-link (subpath")
                    || line.starts_with("(deny file-write* (literal")
            })
            .collect();
        assert_eq!(
            rules,
            [
                "(deny file-write* file-link (subpath \"/Users/op-1356/plane/tools/hook.sh\"))",
                "(deny file-write* file-link (subpath \"/System/Volumes/Data/Users/op-1356/plane/tools/hook.sh\"))",
                "(deny file-write* (literal \"/Users/op-1356/plane\"))",
                "(deny file-write* (literal \"/System/Volumes/Data/Users/op-1356/plane\"))",
                "(deny file-write* (literal \"/Users/op-1356/tmp\"))",
                "(deny file-write* (literal \"/System/Volumes/Data/Users/op-1356/tmp\"))",
                "(deny file-write* (literal \"/Users/op-1356/plane/tools\"))",
                "(deny file-write* (literal \"/System/Volumes/Data/Users/op-1356/plane/tools\"))",
            ],
            "{written}:\n{profile}"
        );
    }
}

#[test]
fn the_firmlinks_are_the_system_s_list_and_the_known_one_together() {
    // #1356 review F2: an unreadable list falls back to the one compiled in, never to none.
    let known = firmlinks_from(None);
    for link in [
        "/Users",
        "/private",
        "/usr/local",
        "/Volumes",
        "/Applications",
    ] {
        assert!(known.contains(&std::path::PathBuf::from(link)), "{link}");
    }
    assert_eq!(known.len(), 18);
    assert_eq!(firmlinks_from(Some("")), known);
    let read = firmlinks_from(Some("/Extra\tExtra\n/Users\tUsers\n/\t\n"));
    assert!(read.contains(&std::path::PathBuf::from("/Extra")));
    assert_eq!(read.len(), 19);
}

/// A chat's environment as the app sets it: sandboxed or not, on `harness`.
fn chat_env(sandboxed: bool, harness: Option<&str>) -> impl Fn(&str) -> Option<String> {
    let harness = harness.map(str::to_owned);
    move |name: &str| match name {
        crate::hookwire::SANDBOXED_ENV if sandboxed => Some("1".to_owned()),
        crate::hookwire::HARNESS_ENV => harness.clone(),
        _ => None,
    }
}

#[test]
fn a_command_a_sandboxed_chat_runs_is_held_to_its_sandbox_on_every_harness() {
    for harness in [Some("claude-code"), Some("codex"), Some("opencode"), None] {
        assert!(
            writes_are_sandboxed_in(&chat_env(true, harness), Started::ByTheChat),
            "{harness:?}"
        );
        assert!(
            !writes_are_sandboxed_in(&chat_env(false, harness), Started::ByTheChat),
            "{harness:?}"
        );
    }
}

#[test]
fn a_hook_is_held_to_the_chat_s_sandbox_only_where_the_harness_s_sandbox_holds_its_hooks() {
    // #1421: Claude Code's sandbox confines its Bash tool, and its hooks run outside it, so a
    // refusal one meets there is not the sandbox's. Codex and opencode run whole inside the wrap.
    // The variable holds the profile's registry name, as a chat's start writes it.
    let hook = |harness| writes_are_sandboxed_in(&chat_env(true, harness), Started::ByTheHarness);
    assert!(!hook(Some("claude-code")));
    assert!(hook(Some("codex")));
    assert!(hook(Some("opencode")));
    // A harness this binary does not know, or none named: never blamed on a sandbox.
    assert!(!hook(Some("aider")));
    assert!(
        !hook(Some("claude")),
        "a kind word is not what the variable holds"
    );
    assert!(!hook(None));
    assert!(!writes_are_sandboxed_in(
        &chat_env(false, Some("codex")),
        Started::ByTheHarness
    ));
}

#[test]
fn every_harness_says_whether_its_sandbox_holds_what_it_starts() {
    for harness in crate::harness::Harness::ALL {
        let holds = harness.adapter().sandbox_holds_what_it_starts();
        // A wrap charter applies is around the whole harness; Claude Code's own is not.
        let wrapped = !matches!(harness, crate::harness::Harness::ClaudeCode);
        assert_eq!(holds, wrapped, "{harness:?}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn the_reporting_socket_s_folder_is_denied_under_every_name_it_has() {
    // #1418: as written, as the kernel resolves it, and on the data volume's other side.
    let (_plane, denied) = denied_with(None, Os::MacOs);
    let settings = claude::settings(&compiled(denied, Os::MacOs)).expect("compiles");
    let socket = std::path::Path::new("/tmp/purlis-1418-absent/app/hooks.sock");
    let mut reporting = settings.reporting_on(Some(socket));
    let names = [
        "/tmp/purlis-1418-absent/app",
        "/private/tmp/purlis-1418-absent/app",
        "/System/Volumes/Data/private/tmp/purlis-1418-absent/app",
    ];
    assert_eq!(
        deny_write(&reporting).split_off(deny_write(&settings).len()),
        names
    );
    let edits: Vec<String> = names
        .iter()
        .flat_map(|name| [format!("Edit(/{name})"), format!("Edit(/{name}/**)")])
        .collect();
    assert_eq!(reporting.deny.split_off(settings.deny.len()), edits);
    // The one socket a chat may reach is still the one the kernel names.
    assert_eq!(
        reporting.sandbox["network"]["allowUnixSockets"],
        serde_json::json!(["/private/tmp/purlis-1418-absent/app/hooks.sock"])
    );
}

#[cfg(unix)]
#[test]
fn a_case_swapped_link_beside_a_folder_does_not_make_its_volume_fold_case() {
    // #1418: the probe asks the folder itself, never what a link of the swapped name reaches.
    let base = tempfile::tempdir().expect("a base");
    let base = base.path().canonicalize().expect("the base");
    std::fs::create_dir_all(base.join("plane")).expect("the plane");
    if std::os::unix::fs::symlink(base.join("plane"), base.join("PLANE")).is_err() {
        // A volume that folds case already has `PLANE`: nothing to tell apart here.
        return;
    }
    let ground = [base.join("plane/new/repo")];
    let denied = [later_code(
        base.join("plane/NEW").to_str().expect("a path"),
        "/f",
        "NEW",
    )];
    assert_eq!(covering(&denied, &ground), None);
}

#[test]
fn a_profile_too_large_to_hand_seatbelt_is_refused_by_name() {
    // #1418: the profile goes in argv, which holds 1 MiB with the environment. One config at
    // its cap fits; a profile past `MOST_PROFILE` is refused with a reason, never an E2BIG.
    let cwd = std::path::Path::new("/opt/purlis-1418/plane");
    let tmp = std::path::Path::new("/opt/purlis-1418/tmp");
    let denied = |count: usize| -> Vec<Denial> {
        (0..count)
            .map(|at| {
                later_code(
                    &format!("/opt/purlis-1418/elsewhere/a-folder-name/{at:06}/hook.sh"),
                    "/f",
                    "x",
                )
            })
            .collect()
    };
    let one = seatbelt::profile(
        &denied(planted::MOST_NAMED),
        &seatbelt::Own::default(),
        cwd,
        tmp,
        &[4040],
        None,
    )
    .expect("one config at its cap");
    assert!(one.len() <= seatbelt::MOST_PROFILE, "{}", one.len());
    assert_eq!(
        seatbelt::profile(
            &denied(16 * planted::MOST_NAMED),
            &seatbelt::Own::default(),
            cwd,
            tmp,
            &[4040],
            None,
        ),
        Err(seatbelt::TOO_LARGE)
    );
    // The way out, after what happened, as every refusal names one.
    assert_eq!(
        seatbelt::not_started(
            "purlis could not start this chat.",
            seatbelt::TOO_LARGE,
            true
        ),
        "purlis could not start this chat. purlis cannot hand the sandbox a profile this long: \
         the project's configs name more paths than one chat's sandbox can hold, so nothing was \
         started. Have the configs name fewer scripts, or start this chat without the sandbox \
         from the new-chat picker."
    );
    assert_eq!(
        seatbelt::not_started("Lead.", seatbelt::CONTROL, true),
        format!("Lead. {}, so nothing was started.", seatbelt::CONTROL)
    );
}

/// #1708: a command run for a chat decides a connection as the chat does, by the layer that
/// lists the host (persona, yours, this chat's), so its record says which; a host the run was
/// only told of by name, with no layer kept, is decided as an Open host, as before.
#[test]
fn a_run_for_a_chat_decides_a_connection_by_the_chats_own_layers() {
    use reach::{By, Decision};
    let confines = Confines {
        hosts: vec![
            "db.example.com:5432".to_owned(),
            "grafana.example.com".to_owned(),
            "named.example.com".to_owned(),
        ],
        reach: reach::Reach::of(vec![
            ("db.example.com:5432".to_owned(), By::You),
            ("grafana.example.com".to_owned(), By::Persona),
        ]),
        ..Confines::default()
    };
    let decides = confines.decides();
    assert_eq!(
        decides.decide("db.example.com", 5432, &[443], &[]),
        Decision::Allowed(By::You)
    );
    assert_eq!(
        decides.decide("grafana.example.com", 443, &[443], &[]),
        Decision::Persona
    );
    assert_eq!(
        decides.decide("named.example.com", 443, &[443], &[]),
        Decision::Open
    );
    assert_eq!(
        decides.decide("other.example.com", 443, &[443], &[]),
        Decision::Ask
    );
    // A host allowed live since the chat started joins at the scope the person chose.
    let mut now = confines.clone();
    now.allow_live("live.example.com", By::Chat);
    assert_eq!(
        now.decides().decide("live.example.com", 443, &[443], &[]),
        Decision::Allowed(By::Chat)
    );
    assert!(now.hosts.contains(&"live.example.com".to_owned()));
}

/// #1708: what a chat's sandbox compiled is what a run for it is held to: the same layers.
#[test]
fn a_chats_confines_keep_the_layers_its_sandbox_was_compiled_with() {
    let root = tempfile::tempdir().expect("a project");
    let policy = Policy {
        hosts: vec![hosts::Host::parse("api.example.com").expect("a host")],
        ..policy_of(&[], false)
    };
    let applied = applied_for(
        Harness::ClaudeCode,
        &policy,
        &Plane::of(None),
        root.path(),
        &machine(Os::MacOs),
    )
    .expect("compiles");
    let confines = applied.confines();
    assert_eq!(
        confines
            .decides()
            .decide("api.example.com", 443, &[443], &[])
            .word(),
        "open"
    );
    assert_eq!(confines.reach.hosts(), confines.hosts);
}

// -------------------------------------------------------------------------------------
// What a Seatbelt profile lets a chat write, read off its text (#1408)
// -------------------------------------------------------------------------------------

/// Whether `profile` lets a chat write `path`, which is written as the kernel names it: the
/// last `file-write*` rule whose path filter matches decides, as Seatbelt's does, and nothing
/// is written where none matches (`(deny default)`). A rule held to a kind of file
/// (`require-all`) is left out, so this answers for an ordinary file.
pub(super) fn writes(profile: &str, path: &std::path::Path) -> bool {
    let path = path.display().to_string();
    // A floor profile starts from everything allowed (#2071).
    let mut verdict = profile.contains("(allow default)");
    for form in forms(profile) {
        let allow = form.starts_with("(allow ");
        if !(allow || form.starts_with("(deny ")) || form.contains("require-all") {
            continue;
        }
        let head = form.split('(').nth(1).unwrap_or_default();
        if !head.split_whitespace().any(|op| op == "file-write*") {
            continue;
        }
        if filters(&form)
            .iter()
            .any(|(kind, value)| matches(kind, value, &path))
        {
            verdict = allow;
        }
    }
    verdict
}

/// Each top-level form of `profile`, whole across its lines.
fn forms(profile: &str) -> Vec<String> {
    let (mut out, mut form, mut depth, mut quoted, mut escaped) =
        (Vec::new(), String::new(), 0_i32, false, false);
    for c in profile.chars() {
        if depth > 0 || c == '(' {
            form.push(c);
        }
        if quoted {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    out.push(std::mem::take(&mut form));
                }
            }
            _ => {}
        }
    }
    out
}

/// Each `(subpath "…")`, `(literal "…")` and `(regex "…")` filter in `form`, its string read
/// back.
fn filters(form: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for kind in ["subpath", "literal", "regex"] {
        for (at, _) in form.match_indices(&format!("({kind} \"")) {
            let mut value = String::new();
            let mut escaped = false;
            for c in form[at + kind.len() + 3..].chars() {
                match (escaped, c) {
                    (true, c) => {
                        value.push(c);
                        escaped = false;
                    }
                    (false, '\\') => escaped = true,
                    (false, '"') => break,
                    (false, c) => value.push(c),
                }
            }
            out.push((kind, value));
        }
    }
    out
}

fn matches(kind: &str, value: &str, path: &str) -> bool {
    match kind {
        "subpath" => path == value || path.starts_with(&format!("{value}/")),
        "literal" => path == value,
        _ => regex::Regex::new(value).is_ok_and(|it| it.is_match(path)),
    }
}

#[test]
fn the_profile_reader_takes_the_last_rule_that_matches() {
    let profile = "(version 1)\n(deny default)\n(allow file-write*\n  (subpath \"/c\")\n  \
                   (literal \"/dev/null\"))\n(deny file-write* (subpath \"/c/held\"))\n\
                   (deny file-write* (regex \"^/c/(.*/)?\\\\.git/config$\"))\n\
                   (deny file-write-create (require-all (vnode-type DIRECTORY) (subpath \"/c\")))\n";
    for (path, want) in [
        ("/c/a.txt", true),
        ("/c", true),
        ("/dev/null", true),
        ("/c/held", false),
        ("/c/held/x", false),
        ("/c/x/.git/config", false),
        ("/c/x/.gitxconfig", true),
        ("/cx", false),
        ("/elsewhere", false),
    ] {
        assert_eq!(writes(profile, std::path::Path::new(path)), want, "{path}");
    }
}

/// Spec #1330's testing rule, for the brokered writes (ADR 0067 §2, #1408): a chat in a clone
/// writing one of the project's own files directly, where the app writes it for the chat, is
/// refused by its sandbox on every harness, the block is told as one on the project's own
/// files, and its Notice names the command that writes the file instead. One row per brokered write: session records, the three memories, todos, the vision
/// and sections of `workspace.md`, persona files, and a new workspace.
#[test]
fn a_chat_in_a_clone_writing_a_brokered_file_directly_is_refused_on_every_harness() {
    let dir = tempfile::tempdir().expect("a plane");
    let root = dir.path().canonicalize().expect("resolved");
    let cwd = root.join("workspaces/alpha/api");
    let tmp = tempfile::tempdir().expect("a temp directory");
    let tmp = tmp.path().canonicalize().expect("resolved");
    std::fs::create_dir_all(&cwd).expect("a clone");
    let shared = crate::contain::SHARED_PERSONA;
    // Each row: what it is, its file, and the command the refusal names instead (`None` where
    // no brokered write makes it yet: a persona's own file).
    let brokered = [
        (
            "a session record",
            "workspaces/alpha/sessions/2026-10-12-x.md".to_owned(),
            Some("`purlis session record`"),
        ),
        (
            "a plane-root session record",
            "sessions/2026-10-12-x.md".to_owned(),
            Some("`purlis session record`"),
        ),
        (
            "workspace memory",
            "workspaces/alpha/memory/a-note.md".to_owned(),
            Some("`purlis workspace remember"),
        ),
        (
            "workspace memory's index",
            "workspaces/alpha/memory/MEMORY.md".to_owned(),
            Some("`purlis workspace remember"),
        ),
        (
            "persona memory",
            "personas/steward/memory/a-note.md".to_owned(),
            Some("`purlis persona remember`"),
        ),
        (
            "shared memory",
            format!("personas/{shared}/memory/a-note.md"),
            Some("`--shared`"),
        ),
        (
            "a todo",
            "workspaces/alpha/todos/a-todo.md".to_owned(),
            Some("`purlis workspace todo"),
        ),
        (
            "the vision and sections",
            "workspaces/alpha/workspace.md".to_owned(),
            Some("`purlis workspace vision"),
        ),
        (
            "a persona file",
            "personas/steward/persona.md".to_owned(),
            None,
        ),
        (
            "a new workspace",
            "workspaces/beta/workspace.md".to_owned(),
            Some("`purlis workspace create <name>"),
        ),
    ];
    let denied = Denied::of(&root, &machine(Os::MacOs));
    let profile = seatbelt::profile(
        &denied.paths,
        &seatbelt::Own::default(),
        &cwd,
        &tmp,
        &[4040],
        None,
    )
    .expect("a profile");
    let settings = claude::settings(&compiled(denied, Os::MacOs)).expect("compiles");
    // Claude Code's sandbox lets a command write the chat's folder and what `allowWrite`
    // names, and nothing else.
    let allowed: Vec<String> = settings.sandbox["filesystem"]
        .get("allowWrite")
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|it| it.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let place = crate::sandboxblock::Place {
        root: &root,
        chat: &cwd,
        cwd: &cwd,
        home: None,
    };
    assert!(writes(&profile, &cwd.join("src/main.rs")), "{profile}");
    for (what, file, tool) in brokered {
        let path = root.join(&file);
        assert!(!writes(&profile, &path), "the wrap writes {what}: {file}");
        assert!(!path.starts_with(&cwd), "{what}");
        assert!(
            !allowed.iter().any(|it| path.starts_with(it)),
            "Claude Code's sandbox writes {what}: {allowed:?}"
        );
        assert_eq!(
            crate::sandboxblock::kind_of(&path, &place),
            crate::sandboxblock::Kind::ProjectFiles,
            "{what}"
        );
        // And the refusal names the tool to use instead (ADR 0067 §2's "direct edits are
        // refused, with the way round").
        let way = crate::sandboxblock::way_out(&path, &root);
        match tool {
            Some(tool) => assert!(way.is_some_and(|way| way.contains(tool)), "{what}: {way:?}"),
            None => assert_eq!(way, None, "{what}"),
        }
    }
}

/// #1828: the program a persona's tool runs is searched for as the chat starts outside every
/// folder the chat may write, and the cache homes its sandbox widens to are among them: what
/// [`Applied::writable`] answers holds every folder a brokered run of the chat drops
/// ([`crate::secrets::brokered::chat_writes`]), so a program a chat plants in its cache home is
/// never the one found for a tool.
#[test]
fn the_program_search_at_a_chat_s_start_skips_the_cache_homes_it_writes() {
    let machine = mac_with(&[]);
    for harness in [Harness::ClaudeCode, Harness::Opencode, Harness::Codex] {
        let applied = under_presets(harness, &[Preset::Toolchains], &machine);
        let caches = applied
            .confines()
            .widened
            .caches
            .clone()
            .expect("the toolchains preset widens to the project's caches");
        let writable = applied.writable();
        for folder in caches.trees.iter().chain(caches.bare.iter()) {
            assert!(
                writable.contains(folder),
                "{harness:?}: {} is not skipped: {writable:?}",
                folder.display()
            );
        }
        // So a search path naming a cache home keeps only the folders no chat writes.
        let planted = caches.trees[0].join("bin");
        assert_eq!(
            super::tools::path_outside(
                &format!("{}:/usr/bin", planted.display()),
                StdPath::new(PROJECT),
                &writable
            ),
            "/usr/bin",
            "{harness:?}"
        );
    }
}

/// #873: no chat sandbox writes the event log's files, `<data>/events/<device>`, though the cache
/// homes it may write live in the same data home (`<data>/cache-homes/<key>`).
#[test]
fn no_chat_sandbox_writes_the_event_log_beside_the_cache_homes_it_may_write() {
    let machine = mac_with(&[]);
    let data = Homes::charter_data(&machine).expect("a data home");
    let events = data
        .join(crate::eventlog::EVENTS)
        .join("0123456789abcdef")
        .join("events.jsonl");
    let cache = cache_home(&machine, StdPath::new(PROJECT)).join("npm/x");
    assert!(cache.starts_with(&data), "{}", cache.display());
    for harness in [Harness::ClaudeCode, Harness::Opencode, Harness::Codex] {
        let applied = under_presets(harness, &Preset::ALL, &machine);
        let writable = applied.writable();
        assert!(
            writable.iter().any(|dir| cache.starts_with(dir)),
            "{harness:?}: the cache home is not written: {writable:?}"
        );
        assert!(
            !writable.iter().any(|dir| events.starts_with(dir)),
            "{harness:?}: the event log is written: {writable:?}"
        );
        if harness == Harness::ClaudeCode {
            let allowed: Vec<StdPathBuf> = claude_sandbox(&applied)["filesystem"]["allowWrite"]
                .as_array()
                .expect("a list")
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|it| StdPathBuf::from(it.split("/*").next().unwrap_or(it)))
                .collect();
            assert!(
                allowed.iter().any(|dir| cache.starts_with(dir)),
                "{allowed:?}"
            );
            assert!(
                !allowed.iter().any(|dir| events.starts_with(dir)),
                "Claude Code writes the event log: {allowed:?}"
            );
        } else {
            let profile = wrap_profile(&applied);
            assert!(writes(&profile, &cache), "{harness:?}:\n{profile}");
            for file in [events.clone(), events.with_file_name("args.key")] {
                assert!(
                    !writes(&profile, &file),
                    "{harness:?} writes {}:\n{profile}",
                    file.display()
                );
            }
        }
    }
}

/// #2009: no chat sandbox writes the browser engines purlis installed,
/// `<data>/engines/chrome-for-testing/`, which the session host runs outside every sandbox, though
/// the cache homes it may write live in the same data home (ADR 0067 §5 class 2).
#[test]
fn no_chat_sandbox_writes_the_browser_engines_beside_the_cache_homes_it_may_write() {
    let machine = mac_with(&[]);
    let data = Homes::charter_data(&machine).expect("a data home");
    let engine = data
        .join("engines/chrome-for-testing/155.0.8059.79-mac-arm64")
        .join("chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing");
    let update = data.join("engines/chrome-for-testing/update.json");
    let cache = cache_home(&machine, StdPath::new(PROJECT)).join("npm/x");
    for harness in [Harness::ClaudeCode, Harness::Opencode, Harness::Codex] {
        let applied = under_presets(harness, &Preset::ALL, &machine);
        let writable = applied.writable();
        assert!(
            writable.iter().any(|dir| cache.starts_with(dir)),
            "{harness:?}: the cache home is not written: {writable:?}"
        );
        for engine_file in [&engine, &update] {
            assert!(
                !writable.iter().any(|dir| engine_file.starts_with(dir)),
                "{harness:?}: {} is written: {writable:?}",
                engine_file.display()
            );
        }
    }
}

// -------------------------------------------------------------------------------------
// What no chat reads is what the window's own file reads leave out (issue 1916, ruling F1)
// -------------------------------------------------------------------------------------

/// The list the plane root's files are read against is the read half of a chat's denials,
/// with what waits to be told to a chat: one list, so the two cannot drift apart. A path a
/// later change denies a chat's reads is kept from the window's tree by the same line.
#[test]
fn what_the_window_keeps_from_its_file_reads_is_every_path_no_chat_reads() {
    let plane = tempfile::tempdir().expect("a plane");
    let machine = machine(Os::MacOs);
    let denied = Denied::of(plane.path(), &machine).and_what_waits_for_a_chat(plane.path());
    let unread: Vec<std::path::PathBuf> = denied
        .paths
        .iter()
        .filter(|it| it.access == Access::ReadWrite)
        .map(|it| it.path.clone())
        .collect();
    assert!(!unread.is_empty());

    let kept = kept_from_reading(plane.path(), &machine).expect("the list is told whole");

    for path in &unread {
        assert!(kept.contains(path), "{path:?} is not kept: {kept:?}");
        assert!(
            kept.contains(&real(path)),
            "{path:?} as the kernel names it"
        );
    }
    // And nothing a chat may read: every kept path is one of those, in one spelling or the
    // other.
    for path in &kept {
        assert!(
            unread.iter().any(|it| it == path || &real(it) == path),
            "{path:?} is kept and a chat reads it"
        );
    }
    // The classes a person would name: a vault, the key, a spool, a dispatch's record, a
    // waiting report, the daemon's folder and the forge's kept answers.
    let said: Vec<String> = kept.iter().map(|it| it.display().to_string()).collect();
    for part in [
        "/vaults",
        "/fingerprint.key",
        "/app/spool",
        "/app/dispatches",
        "/handbacks",
        "/forge-etags",
        "/.config/op",
    ] {
        assert!(said.iter().any(|it| it.ends_with(part)), "{part}: {said:?}");
    }
}

/// The vaults and the key are a chat's to neither read nor write under every spelling of the
/// state folder, there or not, as the other records purlis keeps there are; each path once.
#[test]
fn the_vaults_and_the_key_are_denied_under_every_spelling_of_the_state_folder() {
    let plane = tempfile::tempdir().expect("a plane");
    let denied = Denied::of(plane.path(), &machine(Os::MacOs));
    let unread = paths(&denied, Class::Vaults, Access::ReadWrite);
    for state in crate::names::STATE_DIR.spellings() {
        for kept in ["vaults", crate::secrets::fingerprint::KEY_FILE] {
            let path = plane.path().join(state).join(kept);
            assert_eq!(
                unread.iter().filter(|it| **it == path).count(),
                1,
                "{path:?} in {unread:?}"
            );
        }
    }
}

/// A list that may be short is no list: where the vault registry does not read, or a vault in
/// it does not say where its file is, the window's file reads are told why and given nothing.
#[test]
fn the_windows_list_is_refused_where_a_vaults_storage_cannot_be_told() {
    let machine = machine(Os::MacOs);
    let registry = |text: &str| {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(plane.path().join("vaults.json"), text).expect("a registry");
        let kept = kept_from_reading(plane.path(), &machine).map_err(|untold| untold.why);
        (plane, kept)
    };

    let (plane, kept) = registry("{ not json");
    let file = plane.path().join("vaults.json");
    assert_eq!(
        kept,
        Err(format!(
            "the vault registry {} could not be read",
            file.display()
        ))
    );

    let (plane, kept) =
        registry(r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": 7}}}}"#);
    let file = plane.path().join("vaults.json");
    assert_eq!(
        kept,
        Err(format!(
            "the vault 'dev' in the vault registry {} does not say where its file is",
            file.display()
        ))
    );

    // A registry that reads, with a vault that says where its file is, is told whole.
    let (plane, kept) = registry(
        r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "secrets/dev.json"}}}}"#,
    );
    let kept = kept.expect("the list is told whole");
    assert!(
        kept.contains(&plane.path().join("secrets/dev.json")),
        "{kept:?}"
    );
}

#[test]
fn claude_codes_held_names_match_in_either_case() {
    // A disk that compares names case-folded opens `.GIT/HOOKS` as `.git/hooks`: each name
    // Claude Code holds by glob is written with a class of both cases for every letter.
    let settings = claude::settings(&compiled(Denied::default(), Os::MacOs)).expect("compiles");
    let raw: Vec<String> = settings.sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|it| it.as_str().expect("a path").to_owned())
        .collect();
    assert!(
        raw.contains(&"**/.[gG][iI][tT]/[hH][oO][oO][kK][sS]".to_owned()),
        "{raw:?}"
    );
    assert!(
        raw.contains(&"/**/.[pP][uU][rR][lL][iI][sS]/[vV][aA][uU][lL][tT][sS]/**".to_owned()),
        "{raw:?}"
    );
    assert_eq!(claude::either_case("a.B"), "[aA].[Bb]");
    assert_eq!(
        claude::uncased(&claude::either_case("Library/LaunchAgents")),
        "Library/LaunchAgents"
    );
    assert_eq!(
        claude::uncased(&claude::either_case(".git/hooks")),
        ".git/hooks"
    );
}

// -------------------------------------------------------------------------------------
// Your own level (#2071, ADR 0096): a confirmed `off` starts the chat as the opt-out does
// -------------------------------------------------------------------------------------

/// A sandboxed project whose `purlis.local.toml` says `local`.
fn plane_with_local(local: &str) -> tempfile::TempDir {
    let plane = plane_saying(ON);
    std::fs::write(plane.path().join(crate::profiles::LOCAL_FILE), local).expect("the local file");
    plane
}

fn decided_as(
    plane: &std::path::Path,
    persona: Option<&str>,
) -> Result<Option<Decided>, NotStarted> {
    decide(
        Harness::ClaudeCode,
        plane,
        &machine(Os::MacOs),
        &|_| true,
        None,
        persona,
    )
}

fn your_level() -> Lifted {
    Lifted {
        by: By::YourLevel(Via::Window),
        reason: None,
    }
}

/// [`decide_granted`] for a start made `made`, the chat's level read as `level_as`.
fn decided_made(
    plane: &std::path::Path,
    level_as: Option<&str>,
    made: &Made,
) -> Result<Option<Decided>, NotStarted> {
    decide_granted(
        Harness::ClaudeCode,
        plane,
        &machine(Os::MacOs),
        &|_| true,
        &Person {
            opt_out: None,
            level_as,
            made,
        },
        level_as,
        &grant::Grants::default(),
    )
}

fn asked(how: Asking, asker: Option<&str>) -> Made {
    asked_by(how, &[asker])
}

/// Asked for through the chain `above`, nearest first, the chat the person started last.
fn asked_by(how: Asking, above: &[Option<&str>]) -> Made {
    Made::Asked {
        how,
        above: Some(above.iter().map(|one| one.map(str::to_owned)).collect()),
    }
}

fn sandboxed(decided: &Result<Option<Decided>, NotStarted>) -> bool {
    matches!(decided, Ok(Some(Decided::Sandboxed(_))))
}

#[test]
fn a_confirmed_off_starts_the_chat_without_the_sandbox_as_your_setting() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();

    assert_eq!(
        decided_as(plane.path(), None),
        Ok(Some(Decided::Unsandboxed(your_level())))
    );
    assert!(
        your_level().notice().contains("your setting"),
        "{}",
        your_level().notice()
    );
    // Audited as the person's, and counted as the opt-out is.
    assert_eq!(
        at_start(Some(&your_level()), false, false, true),
        (
            Some(Change::Off(your_level())),
            Some(local::Started::OptedOut)
        )
    );
    assert_eq!(
        Change::Off(your_level()).body(Some(Harness::ClaudeCode), None)["actor"],
        "operator"
    );
}

#[test]
fn an_off_a_chat_wrote_into_the_file_starts_nothing_without_the_sandbox() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"off\"\n");
    for persona in [None, Some("qa")] {
        let decided = decided_as(plane.path(), persona);
        assert!(
            matches!(decided, Ok(Some(Decided::Sandboxed(_)))),
            "{persona:?}: {decided:?}"
        );
    }
}

#[test]
fn a_persona_s_confirmed_off_starts_its_chats_alone_without_the_sandbox() {
    let plane = plane_with_local("[sandbox.personas]\nqa = \"off\"\n");
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Off)).unwrap();
    assert_eq!(
        decided_as(plane.path(), Some("qa")),
        Ok(Some(Decided::Unsandboxed(your_level())))
    );
    let other = decided_as(plane.path(), Some("devops"));
    assert!(
        matches!(other, Ok(Some(Decided::Sandboxed(_)))),
        "{other:?}"
    );
}

#[test]
fn in_a_project_at_the_full_sandbox_on_starts_it_and_floor_starts_the_floor() {
    for (word, at) in [("on", Level::Full), ("floor", Level::Floor)] {
        let plane = plane_with_local(&format!("[sandbox]\nlevel = \"{word}\"\n"));
        let level = personal::Choice::of_word(word);
        personal::set_by_you(plane.path(), None, level).unwrap();
        assert_eq!(
            level_of(&decided_as(plane.path(), None)),
            Some(at),
            "{word}"
        );
    }
}

#[test]
fn a_policy_that_forbids_the_opt_out_keeps_your_floor_at_the_full_sandbox() {
    let plane = plane_with_local("[sandbox]\nlevel = \"floor\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Floor)).unwrap();
    policy::set_for_this_test(policy::Locks::parse(
        r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#,
        std::path::Path::new("policy.json"),
    ));
    let decided = decided_as(plane.path(), None);
    policy::set_for_this_test(policy::Locks::none());
    assert_eq!(level_of(&decided), Some(Level::Full), "{decided:?}");
}

#[test]
fn a_policy_that_forbids_the_opt_out_bounds_your_level_too() {
    let required = policy::Locks::parse(
        r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#,
        std::path::Path::new("policy.json"),
    );
    let off = Some(personal::Choice::Off);
    let plane = plane_saying(ON);
    let held = team::allowed_levels_under(plane.path(), &required);
    let free = team::allowed_levels_under(plane.path(), &policy::Locks::none());
    // Held to the full sandbox (#2071 PR C), the default included.
    assert_eq!(bounded(off, &held), Some(personal::Choice::On));
    assert_eq!(bounded(None, &held), Some(personal::Choice::On));
    assert_eq!(bounded(off, &free), off);
    // Nothing chosen, in a project at the full sandbox: the project's own (#2071), and the floor
    // in one that has not turned it on.
    assert_eq!(bounded(None, &free), Some(personal::Choice::On));
    let unset = plane_saying("schema = 1\n");
    assert_eq!(
        bounded(
            None,
            &team::allowed_levels_under(unset.path(), &policy::Locks::none())
        ),
        Some(personal::Choice::Floor)
    );
    assert_eq!(
        bounded(Some(personal::Choice::On), &held),
        Some(personal::Choice::On)
    );
}

#[test]
fn a_chat_at_on_that_dispatches_or_hands_off_to_a_persona_you_turned_off_starts_it_sandboxed() {
    let plane = plane_with_local("[sandbox]\nlevel = \"on\"\n[sandbox.personas]\nqa = \"off\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::On)).unwrap();
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Off)).unwrap();
    // The person's own start of a qa chat runs without it.
    assert_eq!(
        decided_made(plane.path(), Some("qa"), &Made::Window),
        Ok(Some(Decided::Unsandboxed(your_level())))
    );
    for how in [Asking::Dispatch, Asking::Handoff] {
        let decided = decided_made(plane.path(), Some("qa"), &asked(how, None));
        assert!(sandboxed(&decided), "{how:?}: {decided:?}");
    }
}

#[test]
fn a_chat_at_off_that_dispatches_to_a_persona_at_on_starts_it_sandboxed() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"on\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::On)).unwrap();
    let decided = decided_made(plane.path(), Some("qa"), &asked(Asking::Dispatch, None));
    assert!(sandboxed(&decided), "{decided:?}");
}

#[test]
fn both_at_off_a_chat_asked_for_starts_without_the_sandbox_and_says_who_asked() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    assert_eq!(
        decided_made(plane.path(), None, &asked(Asking::Dispatch, None)),
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::YourLevel(Via::Dispatch),
            reason: None,
        })))
    );
}

#[test]
fn a_start_that_does_not_say_how_it_was_made_or_who_asked_never_turns_the_sandbox_off() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    let unknown = Made::Asked {
        how: Asking::Handoff,
        above: None,
    };
    let empty = Made::Asked {
        how: Asking::Dispatch,
        above: Some(Vec::new()),
    };
    for made in [Made::Unnamed, unknown, empty] {
        let decided = decided_made(plane.path(), None, &made);
        assert!(sandboxed(&decided), "{made:?}: {decided:?}");
    }
    for made in [Made::Resume, Made::Relaunch] {
        let decided = decided_made(plane.path(), None, &made);
        assert!(
            matches!(decided, Ok(Some(Decided::Unsandboxed(_)))),
            "{made:?}: {decided:?}"
        );
    }
}

#[test]
fn the_audit_says_how_a_start_your_level_lifted_was_made() {
    let body = |via| {
        Change::Off(Lifted {
            by: By::YourLevel(via),
            reason: None,
        })
        .body(Some(Harness::ClaudeCode), None)
    };
    let window = body(Via::Window);
    assert_eq!(
        (window["actor"].clone(), window["scope"].clone()),
        ("operator".into(), "local-ui".into())
    );
    assert_eq!(body(Via::Resume)["made"], "resume");
    let relaunch = body(Via::Relaunch);
    assert_eq!(relaunch["made"], "relaunch");
    assert_eq!(relaunch["actor_kind"], "host");
    for (via, word) in [(Via::Dispatch, "dispatch"), (Via::Handoff, "handoff")] {
        let asked = body(via);
        assert_eq!(asked["made"], word);
        assert_eq!(asked["actor_kind"], "agent", "{asked}");
        assert_ne!(asked["scope"], "local-ui", "{asked}");
    }
}

#[test]
fn a_chat_that_names_no_persona_takes_the_project_s_level_not_the_default_persona_s() {
    let plane = plane_with_local("[sandbox.personas]\nsteward = \"off\"\n");
    std::fs::write(
        plane.path().join("charter.toml"),
        format!("{ON}\n[persona]\ndefault = \"steward\"\n"),
    )
    .unwrap();
    personal::set_by_you(plane.path(), Some("steward"), Some(personal::Choice::Off)).unwrap();
    // Its grants are the default persona's; its level is the project's.
    let decided = decide_granted(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::MacOs),
        &|_| true,
        &Person {
            opt_out: None,
            level_as: None,
            made: &Made::Window,
        },
        Some("steward"),
        &grant::Grants::default(),
    );
    assert!(sandboxed(&decided), "{decided:?}");
}

#[test]
fn a_chat_reaches_no_further_than_the_strictest_chat_above_it_however_many_hops() {
    // The person's chat runs at `on`; it asks for a chat as qa, whose `off` they confirmed, and
    // that chat asks for one as ops, `off` too.
    let plane = plane_with_local(
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nsteward = \"on\"\nqa = \"off\"\nops = \"off\"\n",
    );
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    for (name, level) in [
        ("steward", personal::Choice::On),
        ("qa", personal::Choice::Off),
        ("ops", personal::Choice::Off),
    ] {
        personal::set_by_you(plane.path(), Some(name), Some(level)).unwrap();
    }
    for how in [Asking::Dispatch, Asking::Handoff] {
        let qa = decided_made(plane.path(), Some("qa"), &asked_by(how, &[Some("steward")]));
        assert!(sandboxed(&qa), "{how:?}: {qa:?}");
        // The second hop: qa's chat asks for ops, and for a chat on no persona.
        for target in [Some("ops"), None] {
            let second = decided_made(
                plane.path(),
                target,
                &asked_by(how, &[Some("qa"), Some("steward")]),
            );
            assert!(sandboxed(&second), "{how:?} {target:?}: {second:?}");
        }
    }
}

#[test]
fn a_chain_of_chats_all_at_off_starts_without_the_sandbox_and_is_audited_as_a_chat_s() {
    let plane = plane_with_local(
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"off\"\nops = \"off\"\n",
    );
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Off)).unwrap();
    personal::set_by_you(plane.path(), Some("ops"), Some(personal::Choice::Off)).unwrap();
    let decided = decided_made(
        plane.path(),
        Some("ops"),
        &asked_by(Asking::Handoff, &[Some("qa"), None, Some("qa")]),
    );
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::YourLevel(Via::Handoff),
            reason: None,
        })))
    );
    let body = Change::Off(Lifted {
        by: By::YourLevel(Via::Handoff),
        reason: None,
    })
    .body(Some(Harness::ClaudeCode), Some("ops"));
    assert_eq!(body["actor_kind"], "agent", "{body}");
}

#[test]
fn a_chain_entry_naming_the_default_persona_is_held_to_the_project_s_level_too() {
    // A chat on no persona is kept in the chain under the default persona's name.
    let plane = plane_with_local(
        "[sandbox]\nlevel = \"on\"\n[sandbox.personas]\nsteward = \"off\"\nqa = \"off\"\n",
    );
    std::fs::create_dir_all(plane.path().join("personas/steward")).unwrap();
    std::fs::write(
        plane.path().join("personas/steward/persona.md"),
        "# steward\n",
    )
    .unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        format!("{ON}\n[persona]\ndefault = \"steward\"\n"),
    )
    .unwrap();
    personal::set_by_you(plane.path(), None, Some(personal::Choice::On)).unwrap();
    personal::set_by_you(plane.path(), Some("steward"), Some(personal::Choice::Off)).unwrap();
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Off)).unwrap();
    let decided = decided_made(
        plane.path(),
        Some("qa"),
        &asked_by(Asking::Dispatch, &[Some("steward")]),
    );
    assert!(sandboxed(&decided), "{decided:?}");
}

#[test]
fn in_a_project_at_the_floor_your_confirmed_off_starts_the_chat_without_it_and_nothing_else_does() {
    // A project that has not turned the full sandbox on starts its chats at the floor (#2071).
    let plane = plane_saying("schema = 1\n");
    std::fs::write(
        plane.path().join(crate::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"off\"\nsteward = \"floor\"\n",
    )
    .unwrap();
    let at_floor = |decided: &Result<Option<Decided>, NotStarted>| matches!(decided, Ok(Some(Decided::Sandboxed(applied))) if applied.level() == Level::Floor);
    // Written into the file alone: the floor.
    let decided = decided_as(plane.path(), None);
    assert!(at_floor(&decided), "{decided:?}");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    assert_eq!(
        decided_as(plane.path(), None),
        Ok(Some(Decided::Unsandboxed(your_level())))
    );
    // A chat asked for by one whose level is stricter does not start without it.
    personal::set_by_you(plane.path(), Some("steward"), Some(personal::Choice::Floor)).unwrap();
    let asked = decided_made(
        plane.path(),
        Some("qa"),
        &asked(Asking::Dispatch, Some("steward")),
    );
    assert!(at_floor(&asked), "{asked:?}");
}

// ---- #2071 PR C: the team's off, through the one seam a start reads a level by ---------------

/// A project whose committed `charter.toml` turns the sandbox off, the person having accepted
/// that off. Uses git: first run on CI.
fn team_off_accepted(local: &str) -> tempfile::TempDir {
    let plane = plane_saying("[sandbox]\nmode = \"off\"\n");
    std::fs::write(plane.path().join(crate::profiles::LOCAL_FILE), local).expect("local");
    // The local file is this machine's only where git leaves it alone, as in a real project.
    std::fs::write(
        plane.path().join(".gitignore"),
        format!("{}\n", crate::profiles::LOCAL_FILE),
    )
    .expect("gitignore");
    let made = crate::testgit::run(plane.path(), &["init", "-q"]);
    assert!(made.ok(), "{made:?}");
    crate::testgit::run(plane.path(), &["add", "charter.toml", ".gitignore"]);
    let ran = crate::testgit::run(
        plane.path(),
        &[
            "-c",
            "user.name=Ada",
            "-c",
            "user.email=ada@example.invalid",
            "commit",
            "-q",
            "-m",
            "off",
        ],
    );
    assert!(ran.ok(), "{ran:?}");
    assert!(
        matches!(team::team_off(plane.path()), team::TeamOff::Asks(_)),
        "asked"
    );
    team::answer(plane.path(), true).expect("accepted");
    plane
}

#[test]
fn an_accepted_team_off_starts_a_chat_with_no_level_of_yours_without_the_sandbox() {
    let plane = team_off_accepted("");
    let decided = decided_made(plane.path(), None, &Made::Window);
    assert!(
        matches!(decided, Ok(Some(Decided::Unsandboxed(_)))),
        "{decided:?}"
    );
    // A level you chose still stands.
    let plane = team_off_accepted("[sandbox]\nlevel = \"on\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::On)).unwrap();
    assert!(sandboxed(&decided_made(plane.path(), None, &Made::Window)));
}

#[test]
fn a_chat_an_on_asker_dispatched_stays_sandboxed_under_an_accepted_team_off() {
    // #2122's strictest-across-the-chain rule: the team's off never lets a chat-requested
    // start reach past the chat that asked.
    let plane = team_off_accepted("[sandbox.personas]\nqa = \"on\"\n");
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::On)).unwrap();
    let decided = decided_made(
        plane.path(),
        None,
        &asked_by(Asking::Dispatch, &[Some("qa")]),
    );
    assert!(sandboxed(&decided), "{decided:?}");
    // A start that cannot say who asked never takes the team's off either.
    let unnamed = decided_made(plane.path(), None, &Made::Unnamed);
    assert!(
        !matches!(unnamed, Ok(Some(Decided::Unsandboxed(_)))),
        "{unnamed:?}"
    );
}

#[test]
fn under_a_never_policy_a_confirmed_off_starts_the_chat_in_the_full_sandbox() {
    let plane = plane_with_local("[sandbox]\nlevel = \"off\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Off)).unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\npolicy = \"never\"\n",
    )
    .unwrap();
    assert!(sandboxed(&decided_made(plane.path(), None, &Made::Window)));
    assert_eq!(
        super::your_level(
            plane.path(),
            None,
            &policy::Locks::of(plane.path()),
            &Made::Window
        ),
        Some(personal::Choice::On)
    );
}

fn level_of(decided: &Result<Option<Decided>, NotStarted>) -> Option<Level> {
    match decided {
        Ok(Some(Decided::Sandboxed(applied))) => Some(applied.level()),
        _ => None,
    }
}

#[test]
fn your_confirmed_floor_starts_the_floor_in_a_project_at_the_full_sandbox() {
    let plane = plane_with_local("[sandbox]\nlevel = \"floor\"\n");
    // Written into the file alone: the project's full sandbox.
    assert_eq!(level_of(&decided_as(plane.path(), None)), Some(Level::Full));
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Floor)).unwrap();
    assert_eq!(
        level_of(&decided_as(plane.path(), None)),
        Some(Level::Floor)
    );
}

#[test]
fn your_confirmed_on_starts_the_full_sandbox_in_a_project_at_the_floor() {
    let plane = plane_saying("schema = 1\n");
    std::fs::write(
        plane.path().join(crate::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"on\"\n",
    )
    .unwrap();
    // Stricter than the floor, so it counts unconfirmed.
    assert_eq!(level_of(&decided_as(plane.path(), None)), Some(Level::Full));
    std::fs::write(plane.path().join(crate::profiles::LOCAL_FILE), "").unwrap();
    assert_eq!(
        level_of(&decided_as(plane.path(), None)),
        Some(Level::Floor)
    );
}

#[test]
fn a_chat_asked_for_by_one_at_the_full_sandbox_does_not_start_at_the_floor() {
    let plane = plane_with_local("[sandbox.personas]\nqa = \"floor\"\nsteward = \"on\"\n");
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Floor)).unwrap();
    personal::set_by_you(plane.path(), Some("steward"), Some(personal::Choice::On)).unwrap();
    assert_eq!(
        level_of(&decided_made(plane.path(), Some("qa"), &Made::Window)),
        Some(Level::Floor)
    );
    assert_eq!(
        level_of(&decided_made(
            plane.path(),
            Some("qa"),
            &asked(Asking::Dispatch, Some("steward"))
        )),
        Some(Level::Full)
    );
}

#[test]
fn your_on_that_cannot_be_applied_in_a_project_at_the_floor_never_starts_the_chat_unsandboxed() {
    // Chosen stricter than the project's floor: where the full sandbox cannot be applied here,
    // the start is refused with why, as in a project at the full sandbox, and never runs bare.
    let plane = plane_saying("schema = 1\n[sandbox]\negress = []\n");
    std::fs::write(
        plane.path().join(crate::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"on\"\n",
    )
    .unwrap();
    let decided = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Linux),
        &|_| false,
        None,
        None,
    );
    assert!(
        matches!(decided, Err(NotStarted::NoBackend(_))),
        "{decided:?}"
    );
    // Nor where this system has no sandbox at all.
    let windows = decide(
        Harness::ClaudeCode,
        plane.path(),
        &machine(Os::Windows),
        &|_| true,
        None,
        None,
    );
    assert!(
        matches!(windows, Err(NotStarted::NoBackend(_))),
        "{windows:?}"
    );
}

#[test]
fn at_the_floor_a_persona_s_tools_are_the_ones_the_project_declares_in_either_kind_of_project() {
    let tools = |text: &str| {
        floor_personas(&Plane::of(Some(text)))
            .get("devops")
            .map(|grants| grants.tools.len())
    };
    let personas = "[sandbox.personas.devops]\n\
                    tools = { kubectl = { vault = \"devops\", as = \"KUBECONFIG file\", key = \"k\" } }\n";
    // A project that has not turned the full sandbox on, and one that has.
    assert_eq!(tools(&format!("[sandbox]\n{personas}")), Some(1));
    assert_eq!(
        tools(&format!("[sandbox]\nmode = \"on\"\n{personas}")),
        Some(1)
    );
}

#[test]
fn in_a_project_at_the_full_sandbox_a_person_with_no_level_starts_it_and_never_at_the_floor() {
    let plane = plane_saying(ON);
    assert_eq!(level_of(&decided_as(plane.path(), None)), Some(Level::Full));
    assert_eq!(
        level_of(&decided_made(
            plane.path(),
            Some("qa"),
            &asked(Asking::Dispatch, Some("steward"))
        )),
        Some(Level::Full)
    );
}

#[test]
fn under_the_project_s_never_your_floor_starts_the_full_sandbox() {
    let plane = plane_with_local("[sandbox]\nlevel = \"floor\"\n");
    personal::set_by_you(plane.path(), None, Some(personal::Choice::Floor)).unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        format!("{ON}policy = \"never\"\n"),
    )
    .unwrap();
    assert_eq!(level_of(&decided_as(plane.path(), None)), Some(Level::Full));
    assert_eq!(personal::base(plane.path()), personal::Choice::On);
}

#[test]
fn a_chat_asked_for_through_a_chain_purlis_cannot_read_never_starts_lower_than_the_project() {
    // A whole chain holds a dispatch to the asking chat's level; one purlis cannot read whole is
    // never read as a shorter one, so it holds at least to the project's own.
    let plane = plane_with_local("[sandbox.personas]\nqa = \"floor\"\n");
    personal::set_by_you(plane.path(), Some("qa"), Some(personal::Choice::Floor)).unwrap();
    let unread = Made::Asked {
        how: Asking::Dispatch,
        above: None,
    };
    for made in [unread, Made::Unnamed] {
        assert_eq!(
            level_of(&decided_made(plane.path(), Some("qa"), &made)),
            Some(Level::Full),
            "{made:?}"
        );
    }
    // The person's own start of it is theirs.
    assert_eq!(
        level_of(&decided_made(plane.path(), Some("qa"), &Made::Window)),
        Some(Level::Floor)
    );
}

/// SK-9 (#2772): every plane level's skills, shared and local, at the root, in each workspace and
/// in each persona's folder, are denied writing, there yet or not, and those of any workspace or
/// persona made later by pattern; nothing else is.
#[test]
fn the_projects_skills_are_held_at_every_level() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(root.join("personas/flat.md"), "---\nname: flat\n---\n").unwrap();
    let held: Vec<PathBuf> = skills_held(root).into_iter().map(|d| d.path).collect();
    let expected: Vec<PathBuf> = [
        "skills",
        "skills.local",
        "workspaces/ops/skills",
        "workspaces/ops/skills.local",
        "personas/steward/skills",
        "personas/steward/skills.local",
        "workspaces/*/skills",
        "workspaces/*/skills.local",
        "personas/*/skills",
        "personas/*/skills.local",
    ]
    .iter()
    .map(|rel| root.join(rel))
    .collect();
    assert_eq!(held, expected);
    assert!(
        skills_held(root)
            .iter()
            .all(|d| d.class == Class::LaterCode && d.access == Access::Write)
    );
}

/// SK-9 (#2772): purlis's own wrap holds a pattern's levels with a regular expression, each
/// any-name part one name, above it as the kernel names the folder: a workspace made after the
/// chat started cannot be given a skill either.
#[test]
fn a_pattern_denial_is_held_by_a_regular_expression_of_one_name_a_part() {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    std::fs::create_dir_all(root.join("workspaces")).unwrap();
    let denial = Denial {
        class: Class::LaterCode,
        path: root.join("workspaces").join(ANY_NAME).join("skills"),
        access: Access::Write,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    };
    let rules = seatbelt::denial_rules(&denial).unwrap();
    let escaped = seatbelt::escaped(&root.join("workspaces").display().to_string());
    assert!(
        rules.iter().any(|rule| rule
            == &format!(
                "(deny file-write* file-link (regex {}))",
                seatbelt::string(&format!("^{escaped}/[^/]+/skills(/.*)?$")).unwrap()
            )),
        "{rules:?}"
    );
    let regex = regex::Regex::new(&format!("^{escaped}/[^/]+/skills(/.*)?$")).unwrap();
    let at = |rel: &str| root.join(rel).display().to_string();
    assert!(regex.is_match(&at("workspaces/later/skills/x/SKILL.md")));
    assert!(regex.is_match(&at("workspaces/later/skills")));
    assert!(!regex.is_match(&at("workspaces/later/skills-repo/x")));
    assert!(!regex.is_match(&at("workspaces/later/notes/skills/x")));
}

#[test]
fn no_chat_writes_the_folder_chats_servers_listen_in() {
    // A chat that wrote there could take another chat's socket, or the folder its door
    // carries to (#2019); its own socket is given back by the rules that let it serve.
    let (_plane, denied) = denied_with(None, Os::MacOs);
    assert!(
        paths(&denied, Class::Integrity, Access::Write).contains(&door::root_in(None)),
        "{:?}",
        denied.paths
    );
}

#[cfg(unix)]
#[test]
fn whatever_is_left_where_a_servers_socket_goes_is_cleared_but_a_folder() {
    // A chat may make a file at its socket's path (#2019); what it left there never keeps its
    // servers from being served again, and a folder there is refused and said.
    let dir = tempfile::tempdir().expect("a folder");
    let at = dir.path().join("5173.sock");
    assert!(cleared(&at).is_ok(), "nothing there");
    std::os::unix::fs::symlink("/nowhere", &at).expect("a link");
    assert!(cleared(&at).is_ok());
    assert!(std::fs::symlink_metadata(&at).is_err(), "the link is gone");
    std::fs::write(&at, "x").expect("a file");
    assert!(cleared(&at).is_ok());
    assert!(std::fs::symlink_metadata(&at).is_err(), "the file is gone");
    std::fs::create_dir(&at).expect("a folder");
    let refused = cleared(&at).expect_err("a folder is refused");
    assert!(
        refused
            .to_string()
            .contains("a folder is where its socket goes"),
        "{refused}"
    );
}

/// #2019: a port something on this Mac listens on as a chat is compiled is neither served nor
/// reached for a grant to serve on it, and is told.
#[test]
fn a_port_something_listens_on_is_neither_served_nor_reached_for_a_listen_grant() {
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a port in use");
    let port = taken.local_addr().expect("bound").port();
    let widened = Widened::default().and_listens(&[port], &policy::Locks::default());
    assert_eq!(widened.unserved, [port]);
    assert!(widened.listens.is_empty());
    assert!(!widened.ports.contains(&port));
    assert!(widened.listen_only.is_empty());
}

/// The state folder is held whole: its entry is never renamed, moved or removed by a chat in
/// purlis's own wrap, so what purlis holds below it stays where it is held. Claude Code's own
/// sandbox holds only what is below a path, so it is not handed the entry.
#[test]
fn the_state_folder_is_held_whole_in_purlis_s_wrap() {
    let plane = tempfile::tempdir().expect("a plane");
    let state = plane.path().join(".purlis");
    std::fs::create_dir(&state).expect("a state folder");
    let denied = Denied::of(plane.path(), &machine(Os::MacOs));
    let whole: Vec<&Denial> = denied
        .paths
        .iter()
        .filter(|it| it.path == state && it.reach == Reach::Itself)
        .collect();
    assert_eq!(whole.len(), 1, "{:?}", denied.paths);
    assert_eq!(whole[0].class, Class::Integrity);
    assert_eq!(whole[0].access, Access::Write);
    // The spelling not there is still not the chat's to make, as before.
    assert!(
        paths(&denied, Class::Integrity, Access::Write).contains(&plane.path().join(".charter"))
    );
    let rules = seatbelt::denial_rules(whole[0]).expect("rules");

    let mut wrapped_harnesses = 0;
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        let applied = applied_for(
            harness,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &machine(Os::MacOs),
        )
        .expect("compiles");
        match applied.form() {
            Form::ClaudeCode(settings) => {
                let deny_write = settings.sandbox["filesystem"]["denyWrite"].to_string();
                let named = super::real(&state).display().to_string();
                assert!(
                    !deny_write.contains(&format!("\"{named}\"")),
                    "the whole folder is not denied: {deny_write}"
                );
            }
            Form::Codex(_) | Form::Opencode(_) => {
                wrapped_harnesses += 1;
                let profile = seatbelt::profile(
                    applied.form().denied().expect("a wrap"),
                    &seatbelt::Own::default(),
                    plane.path(),
                    std::path::Path::new("/private/tmp/chat"),
                    &[4040],
                    None,
                )
                .expect("a profile");
                for rule in &rules {
                    assert!(profile.contains(rule), "{harness:?}: {rule} missing");
                }
            }
        }
    }
    assert_eq!(wrapped_harnesses, 2);
}

/// #3718: the app's state of every project, in the data home, is purlis's own and read back to
/// decide. No chat writes it, nor renames or replaces it or the data home that holds it, in what
/// every harness is compiled; every chat reads it (a Try it's chat reads its laid-out skill there).
#[test]
fn every_harness_denies_a_chat_writing_the_app_state_and_lets_it_read_it() {
    let plane = tempfile::tempdir().expect("a plane");
    let on = machine(Os::MacOs);
    let data = Homes::charter_data(&on).expect("a data home");
    let app_state = data.join(crate::appstate::DIR);
    let denied = Denied::of(plane.path(), &on);
    let held = |path: &std::path::Path, reach: Reach| {
        denied.paths.iter().any(|it| {
            it.path == path
                && it.reach == reach
                && it.access == Access::Write
                && it.class == Class::Integrity
                && it.any_case
        })
    };
    assert!(held(&app_state, Reach::AndBelow), "{:?}", denied.paths);
    assert!(held(&app_state, Reach::Itself), "{:?}", denied.paths);
    assert!(held(&data, Reach::Itself), "{:?}", denied.paths);
    assert!(
        !denied
            .paths
            .iter()
            .any(|it| it.access == Access::ReadWrite && app_state.starts_with(&it.path)),
        "the app state is denied to reads: {:?}",
        denied.paths
    );

    let named = super::real(&app_state).display().to_string();
    let mut compiled_harnesses = 0;
    for harness in Harness::ALL {
        if compiler(harness).is_none() {
            continue;
        }
        compiled_harnesses += 1;
        let applied = applied_for(
            harness,
            &policy_of(&[], false),
            &Plane::of(None),
            plane.path(),
            &on,
        )
        .expect("compiles");
        match applied.form() {
            // Claude Code: its own sandbox's write denials, and its file tools', and no read
            // denial of it.
            Form::ClaudeCode(settings) => {
                let listed = |key: &str| -> Vec<String> {
                    settings.sandbox["filesystem"][key]
                        .as_array()
                        .map(|all| {
                            all.iter()
                                .filter_map(serde_json::Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let deny_write = listed("denyWrite");
                assert!(deny_write.contains(&named), "{named}: {deny_write:?}");
                assert!(
                    deny_write.contains(&claude::either_case(&named)),
                    "{named} in either case: {deny_write:?}"
                );
                assert!(
                    settings.deny.contains(&format!("Edit(/{named})")),
                    "{named}: {:?}",
                    settings.deny
                );
                assert!(
                    !listed("denyRead")
                        .iter()
                        .any(|denied| named.starts_with(denied.as_str())),
                    "{named} is denied to reads"
                );
                assert!(
                    !settings
                        .deny
                        .iter()
                        .any(|rule| rule.starts_with("Read(") && rule.contains("app-state")),
                    "{:?}",
                    settings.deny
                );
            }
            // Codex and opencode: purlis's own wrap, which holds the folder and the data home as
            // entries too.
            Form::Codex(_) | Form::Opencode(_) => {
                let wrapped = applied.form().denied().expect("a wrap");
                let profile = seatbelt::profile(
                    wrapped,
                    &seatbelt::Own::default(),
                    plane.path(),
                    std::path::Path::new("/private/tmp/chat"),
                    &[4040],
                    None,
                )
                .expect("a profile");
                for denial in app_state_held(&data) {
                    assert!(wrapped.contains(&denial), "{harness:?}: {denial:?}");
                    for rule in seatbelt::denial_rules(&denial).expect("rules") {
                        assert!(profile.contains(&rule), "{harness:?}: {rule} missing");
                    }
                }
                assert!(
                    !profile
                        .lines()
                        .any(|line| line.contains("file-read*") && line.contains("[aA][pP][pP]-")),
                    "{harness:?}: the app state is denied to reads"
                );
            }
        }
    }
    assert_eq!(compiled_harnesses, 3, "Claude Code, Codex and opencode");
}

#[test]
fn every_spelling_of_the_data_home_is_held() {
    // #3718: each folder the data home could be, there or not, under either name.
    let on = machine(Os::Linux);
    let share = on.home.as_ref().expect("a home").join(".local/share");
    let denied = Denied::of(tempfile::tempdir().expect("a plane").path(), &on);
    for name in ["purlis", "charter"] {
        let data = share.join(name);
        assert!(!data.exists());
        for (path, reach) in [
            (data.join(crate::appstate::DIR), Reach::AndBelow),
            (data.join(crate::appstate::DIR), Reach::Itself),
            (data.clone(), Reach::Itself),
        ] {
            assert!(
                denied.paths.iter().any(|it| it.path == path
                    && it.reach == reach
                    && it.access == Access::Write
                    && it.class == Class::Integrity),
                "{path:?} {reach:?}: {:?}",
                denied.paths
            );
        }
    }
}

// -------------------------------------------------------------------------------------
// A repository's git folder, held as a whole
// -------------------------------------------------------------------------------------

/// A plane that turned the sandbox on, with a repository at its top, a nested one with a
/// submodule, a linked worktree of the nested one five folders down (as a workspace keeps its
/// branch folders), and a folder outside it, listed as one chats may be granted, that holds
/// another. Each path as the test lays it out.
struct Repositories {
    plane: tempfile::TempDir,
    granted: tempfile::TempDir,
}

impl Repositories {
    fn laid_out() -> Self {
        let plane = plane_saying(ON);
        let granted = tempfile::tempdir().expect("a folder chats may be granted");
        let repository = |at: &std::path::Path| {
            for dir in [
                "objects/pack",
                "refs/heads",
                "logs/refs/heads",
                "hooks",
                "info",
            ] {
                std::fs::create_dir_all(at.join(".git").join(dir)).expect("a git folder");
            }
            std::fs::write(at.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
            std::fs::write(at.join(".git/config"), "[core]\n\tbare = false\n").expect("config");
        };
        repository(plane.path());
        let nested = plane.path().join("ws/app/repo");
        repository(&nested);
        std::fs::create_dir_all(nested.join(".git/modules/x")).expect("a submodule's git folder");
        std::fs::write(nested.join(".git/modules/x/HEAD"), "ref: refs/heads/main\n")
            .expect("its HEAD");
        std::fs::create_dir_all(nested.join(".git/worktrees/b")).expect("a worktree record");
        std::fs::write(nested.join(".git/worktrees/b/HEAD"), "ref: refs/heads/b\n").expect("HEAD");
        std::fs::write(nested.join(".git/worktrees/b/commondir"), "../..\n").expect("commondir");
        let branch = plane.path().join("workspaces/w/.worktrees/repo/b");
        std::fs::create_dir_all(&branch).expect("a branch folder");
        std::fs::write(
            branch.join(".git"),
            format!("gitdir: {}\n", nested.join(".git/worktrees/b").display()),
        )
        .expect("its .git file");
        repository(&granted.path().join("other"));
        local::list_grantable(plane.path(), granted.path()).expect("listed");
        Self { plane, granted }
    }

    fn at(&self, rel: &str) -> std::path::PathBuf {
        self.plane.path().join(rel)
    }

    /// The path whose denial holds each git folder.
    fn pins(&self) -> Vec<std::path::PathBuf> {
        vec![
            self.at(".git/config"),
            self.at("ws/app/repo/.git/config"),
            self.at("ws/app/repo/.git/modules/x/config"),
            self.at("workspaces/w/.worktrees/repo/b/.git"),
            self.granted.path().join("other/.git/config"),
        ]
    }

    /// The repositories with a `.git` directory.
    fn clones(&self) -> Vec<std::path::PathBuf> {
        vec![
            self.plane.path().to_path_buf(),
            self.at("ws/app/repo"),
            self.granted.path().join("other"),
        ]
    }

    /// The settings a Claude Code chat in the plane starts with, compiled as a start compiles
    /// them, at the full sandbox; and at the floor, from the same denials.
    fn claude_settings(&self) -> Vec<(Level, claude::Settings)> {
        let machine = machine(Os::MacOs);
        let applied =
            compiled_anyway(Harness::ClaudeCode, self.plane.path(), &machine).expect("compiles");
        let Form::ClaudeCode(full) = applied.form_in(Some(self.plane.path())) else {
            panic!("not Claude Code's settings");
        };
        let mut floor = compiled(
            Denied {
                git_folder_pins: git_folder_pins_of(self.plane.path()),
                ..Denied::of(self.plane.path(), &machine)
            },
            Os::MacOs,
        );
        floor.level = Level::Floor;
        let floor = claude::settings(&floor).expect("compiles");
        vec![(Level::Full, full), (Level::Floor, floor)]
    }
}

/// What Claude Code's `denyWrite` `entry` holds, as its profile builder reads one, as
/// `tools/seatbelt-case.sh` describes it: `**/` any folders or none, `**` anything, `*` and `?`
/// within one name, a `[...]` class kept, and what is below the entry held with it. A relative
/// entry is read as matching at any depth, which holds at least as much as Claude Code does.
fn claude_entry_pattern(entry: &str) -> regex::Regex {
    let (mut pattern, body) = match entry.strip_prefix('/') {
        Some(_) => (String::from("^"), entry),
        None => (
            String::from("^/(.*/)?"),
            entry.strip_prefix("**/").unwrap_or(entry),
        ),
    };
    let chars: Vec<char> = body.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        let rest: String = chars[at..].iter().collect();
        if rest.starts_with("**/") {
            pattern.push_str("(.*/)?");
            at += 3;
        } else if rest.starts_with("**") {
            pattern.push_str(".*");
            at += 2;
        } else if chars[at] == '*' {
            pattern.push_str("[^/]*");
            at += 1;
        } else if chars[at] == '?' {
            pattern.push_str("[^/]");
            at += 1;
        } else if chars[at] == '[' {
            let end = chars[at..]
                .iter()
                .position(|c| *c == ']')
                .map_or(chars.len(), |end| at + end + 1);
            pattern.extend(&chars[at..end]);
            at = end;
        } else {
            pattern.push_str(&regex::escape(&chars[at].to_string()));
            at += 1;
        }
    }
    pattern.push_str("(/.*)?$");
    regex::Regex::new(&pattern).expect("a pattern")
}

/// Claude Code's `denyWrite`, each entry as written.
fn deny_write_as_written(settings: &claude::Settings) -> Vec<String> {
    settings.sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|it| it.as_str().expect("a path").to_owned())
        .collect()
}

#[test]
fn every_repository_s_git_folder_is_found_as_a_chat_starts() {
    let repositories = Repositories::laid_out();
    let pins = git_folder_pins_of(repositories.plane.path());
    for pin in repositories.pins() {
        assert!(pins.contains(&pin), "{pin:?} not in {pins:?}");
    }
    // A `.git` directory is never itself a pin: what is below it is git's to write. Nor is a
    // linked worktree's own record, which `.git/worktrees` holds by name.
    for clone in repositories.clones() {
        assert!(!pins.contains(&clone.join(".git")), "{clone:?}");
    }
    assert!(
        !pins.contains(&repositories.at("ws/app/repo/.git/worktrees/b/config")),
        "{pins:?}"
    );
    // Found for a start alone, never with the denials every caller reads.
    assert_eq!(
        Denied::of(repositories.plane.path(), &machine(Os::MacOs)).git_folder_pins,
        Vec::<std::path::PathBuf>::new()
    );
}

/// Measured where it ships by `claude_codes_own_sandbox_holds_the_settings_purlis_writes`
/// (`tools/seatbelt-case.sh --here`): the git folders stay where they are, and git still works.
#[test]
fn a_claude_code_chat_denies_a_path_in_each_repository_s_git_folder() {
    let repositories = Repositories::laid_out();
    for (level, settings) in repositories.claude_settings() {
        let deny_write = deny_write_as_written(&settings);
        for pin in repositories.pins() {
            // As a plain path, never a glob: Claude Code pins every folder above a denied path,
            // so the git folder and the folders that hold it stay where they are.
            for name in [pin.clone(), super::real(&pin)] {
                let name = name.display().to_string();
                assert!(deny_write.contains(&name), "{level:?}: {name} not denied");
                for rule in [format!("Edit(/{name})"), format!("Edit(/{name}/**)")] {
                    assert!(settings.deny.contains(&rule), "{level:?}: {rule}");
                }
            }
        }
    }
}

#[test]
fn git_still_writes_what_is_inside_a_held_git_folder() {
    // Claude Code's `denyWrite` holds a path and what is below it, so no entry may hold a git
    // folder, or what git writes in it, in ordinary work: a commit, a branch, a checkout, a
    // fetch, a gc, a submodule's update.
    let repositories = Repositories::laid_out();
    for (level, settings) in repositories.claude_settings() {
        let deny_write: Vec<(String, regex::Regex)> = deny_write_as_written(&settings)
            .into_iter()
            .map(|entry| {
                let pattern = claude_entry_pattern(&entry);
                (entry, pattern)
            })
            .collect();
        let holding = |path: &std::path::Path| -> Vec<String> {
            let mut out = Vec::new();
            for name in [path.to_path_buf(), super::real(path)] {
                let name = name.display().to_string();
                out.extend(
                    deny_write
                        .iter()
                        .filter(|(_, pattern)| pattern.is_match(&name))
                        .map(|(entry, _)| entry.clone()),
                );
            }
            out
        };
        for repo in repositories.clones() {
            // The control: what git does not write in ordinary work is held, as read here.
            for kept in [".git/config", ".git/hooks/pre-commit"] {
                let path = repo.join(kept);
                assert!(!holding(&path).is_empty(), "{level:?}: {path:?} not held");
            }
            for written in [
                ".git",
                ".git/objects",
                ".git/objects/ab/cdef",
                ".git/objects/pack/pack-1.pack",
                ".git/refs/heads/main",
                ".git/refs/remotes/origin/main",
                ".git/index",
                ".git/index.lock",
                ".git/HEAD",
                ".git/ORIG_HEAD",
                ".git/FETCH_HEAD",
                ".git/COMMIT_EDITMSG",
                ".git/packed-refs",
                ".git/logs/HEAD",
                ".git/logs/refs/heads/main",
                ".git/modules/x/objects/ab/cdef",
                ".git/modules/x/refs/heads/main",
                ".git/modules/x/index",
                ".git/modules/x/HEAD",
            ] {
                let path = repo.join(written);
                let held = holding(&path);
                assert!(held.is_empty(), "{level:?}: {path:?} held by {held:?}");
            }
            // A new linked worktree's record is held by the name `.git/worktrees` alone, as it was:
            // no path in a git folder adds to it.
            let record = repo.join(".git/worktrees/new/HEAD");
            for entry in holding(&record) {
                assert!(
                    claude::uncased(&entry).ends_with(".git/worktrees"),
                    "{level:?}: {record:?} held by {entry}"
                );
            }
        }
    }
}

#[test]
fn a_wrapped_chat_is_compiled_as_before_whatever_git_folders_are_found() {
    // Codex and opencode run in purlis's own wrap, which holds every `.git` by itself already
    // (`Reach::Itself`): what it compiles does not change with the git folders found.
    let repositories = Repositories::laid_out();
    let without = Denied::of(repositories.plane.path(), &machine(Os::MacOs));
    let with = Denied {
        git_folder_pins: git_folder_pins_of(repositories.plane.path()),
        ..without.clone()
    };
    assert!(!with.git_folder_pins.is_empty());
    for os in [Os::MacOs, Os::Linux] {
        assert_eq!(
            codex::wrap(&compiled(with.clone(), os)),
            codex::wrap(&compiled(without.clone(), os)),
            "{os:?}"
        );
        assert_eq!(
            opencode::wrap(&compiled(with.clone(), os)),
            opencode::wrap(&compiled(without.clone(), os)),
            "{os:?}"
        );
    }
}

#[test]
fn a_plane_with_no_repository_has_no_git_folder_pins() {
    let plane = plane_saying(ON);
    assert_eq!(
        git_folder_pins_of(plane.path()),
        Vec::<std::path::PathBuf>::new()
    );
}

#[test]
fn a_block_on_a_git_folder_or_a_folder_above_one_is_known_as_kept_in_place() {
    let repositories = Repositories::laid_out();
    let root = repositories.plane.path();
    for path in [
        repositories.at("ws/app/repo/.git"),
        repositories.at("ws/app/repo/.GIT"),
        repositories.at("ws/app/repo"),
        repositories.at("ws/app"),
        repositories.at("workspaces/w/.worktrees/repo/b/.git"),
        repositories.at("workspaces/w/.worktrees/repo/b"),
        repositories.at("ws/app/repo/.git/modules"),
        repositories.granted.path().join("other/.git"),
        super::real(&repositories.at("ws/app/repo")),
    ] {
        assert!(grant::keeps_git_folder(&path, root, true), "{path:?}");
    }
    for path in [
        repositories.at("ws/app/repo/.git/index"),
        repositories.at("ws/app/repo/src"),
        repositories.at("ws/app/beside"),
        repositories.at("ws/app/repo/.git/worktrees/new"),
    ] {
        assert!(!grant::keeps_git_folder(&path, root, true), "{path:?}");
    }
    // A spelling in another case is the same folder only where names are folded.
    assert!(!grant::keeps_git_folder(
        &repositories.at("ws/app/repo/.GIT"),
        root,
        false
    ));
}
