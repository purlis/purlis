// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn the_scenario_build_migrates_at_launch_only_where_its_one_spec_says_so() {
    // RN-10 (#1268, D-RN5-7): the e2e build skips the launch's rename-local, because its
    // specs name the old folders; the upgrade spec alone turns it on, with `1`.
    assert!(scenario_migrates_at_launch(Some("1")));
    for off in [
        None,
        Some(""),
        Some("0"),
        Some("yes"),
        Some("true"),
        Some(" 1"),
    ] {
        assert!(!scenario_migrates_at_launch(off), "{off:?}");
    }
}

#[test]
fn a_listed_chat_carries_the_id_it_keeps_across_a_launch() {
    // #1687: the window keys the Chats list's folds by what a chat keeps across a launch,
    // and its number is dealt again at each one. A chat put back keeps its id (ADR 0066),
    // so a row the sidebar lists and the chat the launch's list hands back both carry it.
    const KEPT: &str = "01JB6Q9X7ZK3M4N5P6R7S8T9VW";
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    let host = host::pretend::Pretend::default();
    let planes = Planes::telling(
        std::sync::Arc::new(|_: hooks::Moved| {}),
        Shipped::default(),
        None,
    )
    .running_sessions_on(std::sync::Arc::new(move |_, _: &std::path::Path| {
        Box::new(host.clone())
    }));
    let plane = planes.open(&root);
    let held = planes.held(&plane).expect("held");
    let chat = purlis_core::reopen::Chat {
        program: "/nowhere/a-harness".to_owned(),
        name: "one".to_owned(),
        identity: purlis_core::reopen::Identity {
            id: Some(KEPT.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    };
    let session = held
        .chats()
        .start(
            &chat,
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("the host starts it");

    let sidebar = sidebar_of(&held).expect("the sidebar");
    let row = sidebar
        .workspaces
        .into_iter()
        .flat_map(|workspace| workspace.chats)
        .chain(sidebar.unfiled)
        .find(|row| row.session == session)
        .expect("the chat is listed");
    assert_eq!(row.id.as_deref(), Some(KEPT));
    assert_eq!(
        drawn(&held, session).expect("drawn").id.as_deref(),
        Some(KEPT)
    );
}

/// A git repo at a fresh temp dir holding the local settings file and nothing ignoring it.
fn plane_carrying_local_settings() -> tempfile::TempDir {
    let plane = tempfile::tempdir().expect("a plane");
    let git = purlis_core::forklock::status(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(plane.path()),
    )
    .expect("git runs");
    assert!(git.success());
    std::fs::write(purlis_core::names::local_settings(plane.path()), "").expect("written");
    plane
}

#[test]
fn the_picker_names_the_doctors_fix_where_one_ignore_line_cures_the_local_file() {
    // NO-8 (#1233): the picker's refusal offers the `local-ignore` fix the doctor row does.
    let plane = plane_carrying_local_settings();

    let options = start_options_in(plane.path()).expect("read");

    assert!(options.ignore_fix.is_some(), "git would carry the file");
    assert_eq!(options.ignore_fix_id.as_deref(), Some("local-ignore"));
}

#[test]
fn the_picker_names_no_fix_where_an_ignore_line_would_not_cure_it() {
    // Tracked already: an ignore line does not untrack it, and charter never runs `git rm`.
    let plane = plane_carrying_local_settings();
    let local = purlis_core::names::local_settings(plane.path());
    let added = purlis_core::forklock::status(
        std::process::Command::new("git")
            .arg("-C")
            .arg(plane.path())
            .arg("add")
            .arg(&local),
    )
    .expect("git runs");
    assert!(added.success());

    let options = start_options_in(plane.path()).expect("read");

    assert!(options.ignore_fix.is_some(), "git would carry the file");
    assert_eq!(options.ignore_fix_id, None);
}

#[test]
fn the_picker_is_told_each_personas_own_profile_where_the_project_offers_it() {
    // #1445: picking a persona moves the harness row to its own profile.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("written");
    for (name, line) in [
        ("ops", "profile: codex\n"),
        ("qa", ""),
        ("rogue", "profile: sh -c evil\n"),
        ("old", "model: sonnet\n"),
    ] {
        let dir = plane.path().join("personas").join(name);
        std::fs::create_dir_all(&dir).expect("a persona");
        std::fs::write(
            dir.join("persona.md"),
            format!("---\nname: {name}\n{line}---\n"),
        )
        .expect("a definition");
    }

    let options = start_options_in(plane.path()).expect("read");

    assert_eq!(
        options.persona_profiles,
        std::collections::BTreeMap::from([("ops".to_owned(), "codex".to_owned())]),
        "only a profile the project offers is a row to move to"
    );
}

#[test]
fn the_picker_is_told_each_personas_one_line_description_where_it_has_one() {
    // #1460: the description a chat as the persona is briefed with, under its row.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("written");
    for (name, line) in [
        ("ops", "description: Keeps the lights on\n"),
        (
            "qa",
            "description: x\nagent-description: Breaks things on purpose\n",
        ),
        ("bare", ""),
    ] {
        let dir = plane.path().join("personas").join(name);
        std::fs::create_dir_all(&dir).expect("a persona");
        std::fs::write(
            dir.join("persona.md"),
            format!("---\nname: {name}\n{line}---\n"),
        )
        .expect("a definition");
    }

    let options = start_options_in(plane.path()).expect("read");

    assert_eq!(
        options.persona_descriptions,
        Some(std::collections::BTreeMap::from([
            ("ops".to_owned(), "Keeps the lights on".to_owned()),
            ("qa".to_owned(), "Breaks things on purpose".to_owned()),
        ]))
    );
}

#[test]
fn the_picker_shows_a_profile_command_longer_than_the_display_limit_whole() {
    // #1014: the picker's row is the line its approval sentence asks with, and the line
    // `approve_profile` checks the click against, so it carries every word that runs.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("written");
    let filler = "x".repeat(purlis_core::shown::DISPLAY_LIMIT);
    std::fs::write(
        plane.path().join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\n\
             command = [\"claude\", \"{filler}\", \"the-last-word\"]\n"
        ),
    )
    .expect("written");

    let options = start_options_in(plane.path()).expect("read");

    let work = options
        .profiles
        .iter()
        .find(|row| row.name == "work")
        .expect("work is listed");
    assert_eq!(
        work.shown,
        format!("claude {filler} the-last-word (kind claude)")
    );
    assert_eq!(work.approval.as_deref(), Some("new"));
}

/// A store that remembers the plane at `root` with `pins`, in that order.
fn store_pinning(root: &std::path::Path, pins: &[&str]) -> purlis_core::machine::Store {
    let mut store = purlis_core::machine::Store::default();
    store.remember(root, 1);
    for pin in pins {
        store
            .pin_workspace(root, pin, true)
            .expect("the pin is written");
    }
    store
}

#[test]
fn a_pin_whose_workspace_is_gone_is_named_with_its_place_in_the_order() {
    let plane = tempfile::tempdir().expect("a plane");
    for name in ["beta", "gamma"] {
        std::fs::create_dir_all(plane.path().join("workspaces").join(name)).expect("made");
    }
    let store = store_pinning(plane.path(), &["beta", "was-here", "gamma"]);

    let pins = pins_in(&store, plane.path());

    assert_eq!(pins.workspaces, ["beta", "gamma"]);
    assert_eq!(pins.missing, ["was-here"]);
    assert_eq!(pins.order, ["beta", "was-here", "gamma"]);
    assert!(pins.certain, "a whole listing is a certain answer");
}

/// A plane with `beta` and `gamma` under `workspaces/`, and a store pinning `pins`.
fn plane_pinning(pins: &[&str]) -> (tempfile::TempDir, purlis_core::machine::Store) {
    let plane = tempfile::tempdir().expect("a plane");
    for name in ["beta", "gamma"] {
        std::fs::create_dir_all(plane.path().join("workspaces").join(name)).expect("made");
    }
    let store = store_pinning(plane.path(), pins);
    (plane, store)
}

#[test]
fn no_pin_is_named_gone_when_the_workspaces_directory_is_not_there() {
    let (plane, store) = plane_pinning(&["beta", "gamma"]);
    std::fs::remove_dir_all(plane.path().join("workspaces")).expect("removed");

    let pins = pins_in(&store, plane.path());
    assert_eq!(pins.missing, Vec::<String>::new());
    assert!(!pins.certain, "no listing is no answer about what has gone");
}

#[test]
fn no_pin_is_named_gone_when_the_plane_root_is_not_there() {
    let (plane, store) = plane_pinning(&["beta", "gamma"]);
    let root = plane.path().to_path_buf();
    drop(plane);

    let pins = pins_in(&store, &root);

    assert_eq!(pins.missing, Vec::<String>::new());
    assert_eq!(pins.order, ["beta", "gamma"]);
    assert!(!pins.certain);
}

#[cfg(unix)]
#[test]
fn no_pin_is_named_gone_when_the_workspaces_directory_cannot_be_read() {
    use std::os::unix::fs::PermissionsExt;
    let (plane, store) = plane_pinning(&["beta", "was-here"]);
    let under = plane.path().join("workspaces");
    std::fs::set_permissions(&under, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    // Root reads anything, so on a runner that is root there is nothing to refuse.
    let refused = std::fs::read_dir(&under).is_err();

    let pins = pins_in(&store, plane.path());

    std::fs::set_permissions(&under, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if refused {
        assert_eq!(pins.missing, Vec::<String>::new());
        assert!(!pins.certain);
    }
}

#[test]
fn neither_name_of_a_rename_in_progress_is_named_gone() {
    // `wscmd::rename` moves the directory first and the pins last, so in between the old
    // name's pin names nothing: its journal says the rename is not done.
    let (plane, store) = plane_pinning(&["beta", "old-name", "new-name"]);
    let journal = purlis_core::wscmd::rename::journal_path(plane.path());
    std::fs::create_dir_all(journal.parent().expect("a parent")).expect("made");
    std::fs::write(
        &journal,
        r#"{"from":"old-name","to":"new-name","moved":true}"#,
    )
    .expect("written");

    let pins = pins_in(&store, plane.path());
    assert_eq!(pins.missing, Vec::<String>::new());
    assert!(!pins.certain, "a rename between its steps is not an answer");

    std::fs::remove_file(&journal).expect("removed");
    let pins = pins_in(&store, plane.path());
    assert_eq!(pins.missing, ["old-name", "new-name"]);
    assert!(pins.certain);
}

#[cfg(unix)]
#[test]
fn a_workspace_whose_symlink_target_is_missing_is_not_named_gone() {
    // D-NO1-9: an unmounted target comes back, so the pin is not charter's to drop.
    let (plane, store) = plane_pinning(&["beta", "mounted", "was-here"]);
    std::os::unix::fs::symlink(
        plane.path().join("not-mounted"),
        plane.path().join("workspaces").join("mounted"),
    )
    .expect("linked");

    let pins = pins_in(&store, plane.path());
    assert_eq!(pins.missing, ["was-here"]);
    assert!(
        !pins.certain,
        "a link whose target is away is not known gone or there"
    );
}

#[test]
fn no_pin_is_named_gone_when_the_workspaces_cannot_be_listed() {
    // The window offers to forget what `missing` names (V91c), so a listing that failed
    // must not read as every workspace having gone.
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("workspaces"), "not a directory").expect("written");
    let store = store_pinning(plane.path(), &["beta", "gamma"]);

    let pins = pins_in(&store, plane.path());

    assert_eq!(pins.missing, Vec::<String>::new());
    assert_eq!(pins.order, ["beta", "gamma"]);
    assert!(!pins.certain);
}

/// Writes `BINDINGS`, for when the commands above change:
/// `cargo test -p purlis-app -- --ignored`.
#[test]
#[ignore = "writes the bindings instead of checking them"]
fn regenerate_the_typescript_the_ui_imports() {
    commands()
        .export(typescript(), BINDINGS)
        .expect("the bindings are written");
    std::fs::write(UI_RPC, ui_rpc_client()).expect("the UI RPC's client is written");
}

/// The bundled plugin's `hooks/hooks.json`, in the repository.
fn hooks_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(PLUGIN_DIR)
        .join(purlis_core::plugin::HOOKS_FILE)
}

/// Writes the bundled plugin's `hooks.json` from `purlis_core::plugin::HOOKS`, for when
/// the registry changes: `cargo test -p purlis-app -- --ignored`.
#[test]
#[ignore = "writes the plugin's hooks file instead of checking it"]
fn regenerate_the_bundled_plugins_hooks() {
    std::fs::write(hooks_file(), purlis_core::plugin::hooks_json())
        .expect("the hooks file is written");
}

#[test]
fn the_bundled_plugins_hooks_are_the_ones_the_registry_generates() {
    // One registry, and the file a chat loads is generated from it — never edited by hand,
    // so a hook cannot be wired that `charter hook` does not answer.
    assert_eq!(
        std::fs::read_to_string(hooks_file()).unwrap_or_default(),
        purlis_core::plugin::hooks_json(),
        "{} is out of date: run `cargo test -p purlis-app -- --ignored`",
        hooks_file().display()
    );
}

/// The bundled opencode shim, in the repository.
fn opencode_shim_file() -> PathBuf {
    purlis_core::opencode::shim_in(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PLUGIN_DIR))
}

/// Writes the bundled opencode shim from `purlis_core::opencode`, for when it changes:
/// `cargo test -p purlis-app -- --ignored`.
#[test]
#[ignore = "writes the opencode shim instead of checking it"]
fn regenerate_the_bundled_opencode_shim() {
    std::fs::write(
        opencode_shim_file(),
        purlis_core::opencode::shim(purlis_core::opencode::Arming::Session),
    )
    .expect("the shim is written");
}

#[test]
fn the_bundled_opencode_shim_is_the_one_the_core_generates() {
    // One source, as for the hooks file: the routing and the words are the core's, so a
    // tool cannot be sent to a word `charter hook` does not answer (#371).
    assert_eq!(
        std::fs::read_to_string(opencode_shim_file()).unwrap_or_default(),
        purlis_core::opencode::shim(purlis_core::opencode::Arming::Session),
        "{} is out of date: run `cargo test -p purlis-app -- --ignored`",
        opencode_shim_file().display()
    );
}

#[test]
fn the_bundled_plugin_is_called_what_the_app_loads_it_as() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(PLUGIN_DIR)
        .join(".claude-plugin/plugin.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest).expect("plugin.json"))
            .expect("plugin.json is JSON");
    assert_eq!(doc["name"], purlis_core::plugin::NAME);
}

#[test]
fn nothing_the_plugin_ships_names_a_skill_by_the_plugins_old_name() {
    // #406: the plugin was `charter-app` until it was renamed `charter`, and a skill named
    // `charter-app:<skill>` is one no chat the app starts has any more.
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PLUGIN_DIR),
        &mut files,
    );
    assert!(!files.is_empty());
    for file in files {
        let text = std::fs::read_to_string(&file).expect("text");
        assert!(!text.contains("charter-app:"), "{}", file.display());
    }
}

#[test]
fn the_bundle_carries_the_plugin_as_a_resource() {
    // `bundled_plugin` looks for it in the resource directory, so a build that did not
    // copy it there would start every chat unarmed.
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json is JSON");
    assert_eq!(
        conf["bundle"]["resources"][format!("{PLUGIN_DIR}/")],
        format!("{PLUGIN_DIR}/")
    );
}

#[test]
fn every_bundle_carries_purlis_s_licence_and_notice() {
    // The Apache License 2.0 asks that every copy carry the licence (§4(a)) and the NOTICE
    // file (§4(d)), so both are resources of the development build and the release build
    // alike; the CLI ships inside the same bundle.
    for (name, conf) in [
        ("tauri.conf.json", include_str!("../tauri.conf.json")),
        (
            "tauri.release.conf.json",
            include_str!("../tauri.release.conf.json"),
        ),
    ] {
        let conf: serde_json::Value = serde_json::from_str(conf).expect("JSON");
        for file in ["LICENSE", "NOTICE"] {
            assert_eq!(
                conf["bundle"]["resources"][format!("../../{file}")],
                file,
                "{name} does not bundle the repository's {file}"
            );
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let licence = std::fs::read_to_string(root.join("LICENSE")).expect("LICENSE");
    assert!(
        licence.contains("Version 2.0, January 2004"),
        "LICENSE is the Apache License"
    );
    let notice = std::fs::read_to_string(root.join("NOTICE")).expect("NOTICE");
    assert!(notice.contains("Copyright 2026 Purlis"));
}

#[test]
fn the_window_is_never_narrower_than_the_title_bar_is_built_for() {
    // ADR 0054, amended 2026-09-26 (charter#403): 1024 px is the narrowest window, and the
    // title bar is held to room for two project tabs there (`title-bar.e2e.ts`). A window
    // the operator could drag narrower would be one nothing promises anything about.
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json is JSON");
    let main = conf["app"]["windows"]
        .as_array()
        .and_then(|windows| windows.iter().find(|one| one["label"] == lifecycle::WINDOW))
        .expect("tauri.conf.json declares the main window");
    assert_eq!(main["minWidth"], 1024);
}

#[test]
fn the_typescript_the_ui_imports_is_the_one_these_commands_generate() {
    let out = tempfile::tempdir().expect("a directory to generate into");
    let generated = out.path().join("bindings.ts");
    commands()
        .export(typescript(), &generated)
        .expect("the bindings are generated");

    assert_eq!(
        std::fs::read_to_string(BINDINGS).unwrap_or_default(),
        std::fs::read_to_string(&generated).expect("the generated bindings are readable"),
        "{BINDINGS} is out of date: run `cargo test -p purlis-app -- --ignored`"
    );
}

#[test]
fn the_ui_rpcs_typescript_client_is_the_one_these_commands_generate() {
    assert_eq!(
        std::fs::read_to_string(UI_RPC).unwrap_or_default(),
        ui_rpc_client(),
        "{UI_RPC} is out of date: run `cargo test -p purlis-app -- --ignored`"
    );
}

#[test]
fn the_ui_rpcs_client_calls_every_command_over_the_link_and_none_over_tauri() {
    let client = ui_rpc_client();
    assert!(client.contains(r#"from "./uiRpcLink";"#));
    assert!(
        !client.contains("@tauri-apps/"),
        "the UI RPC's client imports nothing of Tauri's"
    );
    let (value_free, vault_values) = app_commands!(command_names);
    for command in value_free.iter().chain(vault_values) {
        let called = client.contains(&format!("(\"{command}\""));
        if TAKES_A_CHANNEL.contains(command) {
            assert!(
                !called,
                "`{command}` takes a channel, which a view carries instead"
            );
        } else if purlis_session_protocol::ui::window_only(command) {
            assert!(!called, "`{command}` is the window's alone (HP-6)");
        } else {
            assert!(
                called,
                "`{command}` is in ipc_commands.rs and not in the UI RPC's client"
            );
        }
    }
}

#[test]
fn the_ui_rpcs_client_has_no_command_that_takes_a_channel() {
    // A channel is Tauri's stream to the window; on the link, a terminal's bytes are a
    // view of the session protocol (ADR 0068 §4), so such a command has no UI RPC form.
    let client = ui_rpc_client();
    assert!(
        !client.contains("Channel<"),
        "a command that takes a channel is in the client"
    );
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    for command in TAKES_A_CHANNEL {
        assert!(
            bindings.contains(&format!("(\"{command}\"")),
            "`{command}` is no longer a command of the window's, so nothing need leave it out"
        );
    }
}

#[test]
fn answering_an_ask_is_the_window_s_alone_and_never_in_the_link_s_client() {
    // HP-6, V16, V75: the window's Tauri IPC answers an ask. On the link to `charterd` an
    // answer is the session protocol's `answer`, checked by FD-27's scope table, never a
    // second route through the UI RPC.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    assert!(bindings.contains("(\"answer_ask\""), "the window answers");
    assert!(!ui_rpc_client().contains("(\"answer_ask\""));
}

#[test]
fn ending_a_chat_is_the_window_s_alone_and_never_in_the_link_s_client() {
    // #1488: ending a task writes a sentence in the person's name to the chat that asked,
    // so it is the window's over Tauri's IPC, as answering an ask is; and so is every
    // other command that ends, starts or restarts a chat on the person's word.
    // Stop all tasks and its question too (#1498).
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    let client = ui_rpc_client();
    for command in [
        "end_task",
        "task_ending",
        "stop_all_tasks",
        "all_tasks_ending",
        "stop_chat",
        "close_session",
        "close_chat_stopping",
        "end_task_that_did_not_start",
        "smart_close",
        "stop_every_agent",
        "forget_chat_that_did_not_start",
        "close_plane",
        "forget_project",
        "restart_on_the_session_bus",
        "restart_to_update",
        "ask_persona_chat",
        "start_chat",
        "start_chat_here",
        "open_session",
        "open_shell_in_branch",
        "first_task_run",
        "resume_session",
        "retry_chat_that_did_not_start",
        "reopen_finished_task",
        "curate",
        "theme_chat",
        "relaunch",
        "restart_chat_without_sandbox",
        "restart_chat",
        "start_chat_fresh",
        "ask_chat_restart",
        "move_chat",
        "move_chat_now",
    ] {
        let called = format!("(\"{command}\"");
        assert!(bindings.contains(&called), "the window's: {command}");
        assert!(!client.contains(&called), "served on a link: {command}");
    }
    // Sending a task's tab back to the list ends nothing, and is an ordinary command.
    assert!(client.contains("(\"close_chat_tab\""));
}

#[test]
fn a_task_s_brief_is_read_by_the_window_alone_and_never_in_the_link_s_client() {
    // #1494: a brief is whatever the work held, and the dispatch store is kept from the
    // project's chats. Reading one back is Tauri's IPC, and no second route.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    assert!(bindings.contains("(\"task_brief\""), "the window reads it");
    assert!(!ui_rpc_client().contains("(\"task_brief\""));
}

#[test]
fn several_rows_are_typed_into_a_chat_by_the_window_alone() {
    // #1151: several of the explorer's rows typed into a chat as the person, one paste.
    // Held to the window until typing into a chat has its ruling: a new command of that kind
    // fails closed, so the link's client carries none.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    assert!(
        bindings.contains("(\"references_into_chat\""),
        "the window types them"
    );
    assert!(!ui_rpc_client().contains("(\"references_into_chat\""));
    assert!(purlis_session_protocol::ui::WINDOW_ONLY.contains(&"references_into_chat"));
}

#[test]
fn what_chats_said_to_each_other_is_read_by_the_window_alone_and_never_in_the_link_s_client() {
    // A session's Activity (#1495) and the question a task is paused on (#1496) are words
    // chats wrote, read by a session the caller names: Tauri's IPC, and no second route.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    let client = ui_rpc_client();
    for command in ["activity", "task_question"] {
        let called = format!("(\"{command}\"");
        assert!(bindings.contains(&called), "the window reads it: {command}");
        assert!(!client.contains(&called), "served on a link: {command}");
    }
}

#[test]
fn the_person_s_answer_to_a_task_is_the_window_s_alone_and_never_in_the_link_s_client() {
    // #1496: what `answer_task_question` sends reaches a chat marked as the person's own
    // words, so it is held to what `answer_ask` is: Tauri's IPC, and no second route.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    assert!(
        bindings.contains("(\"answer_task_question\""),
        "the window answers"
    );
    assert!(!ui_rpc_client().contains("(\"answer_task_question\""));
}

#[test]
fn a_refusal_kept_while_nobody_was_there_is_the_window_s_alone_to_read_and_answer() {
    // #1507: a standing grant in one press, offered on a refused chat's word. The window's
    // Tauri IPC reads the list and answers it; the link's client carries none of the four.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    let client = ui_rpc_client();
    for command in [
        "dispatch_away",
        "allow_dispatch_away",
        "dismiss_dispatch_away",
        "never_dispatch_away",
    ] {
        assert!(bindings.contains(&format!("(\"{command}\"")), "{command}");
        assert!(!client.contains(&format!("(\"{command}\"")), "{command}");
        assert!(
            purlis_session_protocol::ui::WINDOW_ONLY.contains(&command),
            "{command}"
        );
    }
}

#[test]
fn every_command_that_changes_a_standing_dispatch_grant_is_the_window_s_alone() {
    // Spec #1483, train 64: the person's consent to who may dispatch to whom is given and
    // withdrawn in their own window. Each such command is one of the window's (so the
    // list names no command that does not exist), and the link's client carries none.
    let bindings = std::fs::read_to_string(BINDINGS).unwrap();
    let client = ui_rpc_client();
    for command in purlis_session_protocol::ui::STANDING_DISPATCH {
        assert!(bindings.contains(&format!("(\"{command}\"")), "{command}");
        assert!(!client.contains(&format!("(\"{command}\"")), "{command}");
    }
    // And every command of the grant store and of the refusals kept while nobody was
    // there is on that list: the reads of what stands and what waits too, and Keep
    // blocked, which keeps nothing past the chat and is the person's answer.
    let served: [&str; 0] = [];
    // Each command's path as the one list writes it, every segment followed by `::`.
    macro_rules! command_paths {
        (
            value_free: [$($($free:ident)::+),* $(,)?],
            vault_values: [$($($value:ident)::+),* $(,)?],
            e2e_only: [$($($e2e:ident)::+),* $(,)?] $(,)?
        ) => {
            (
                &[$(concat!($(stringify!($free), "::"),+)),*] as &[&str],
                &[$(concat!($(stringify!($value), "::"),+)),*] as &[&str],
            )
        };
    }
    let (value_free, vault_values) = app_commands!(command_paths);
    let mut seen = 0;
    for path in value_free.iter().chain(vault_values) {
        let Some(command) = path
            .strip_prefix("dispatchgrants::")
            .or_else(|| path.strip_prefix("dispatchaway::"))
            .and_then(|rest| rest.strip_suffix("::"))
        else {
            continue;
        };
        seen += 1;
        assert_eq!(
            purlis_session_protocol::ui::STANDING_DISPATCH.contains(&command),
            !served.contains(&command),
            "{command}"
        );
    }
    assert_eq!(
        seen,
        purlis_session_protocol::ui::STANDING_DISPATCH.len() + served.len(),
        "every command of the two modules was looked at"
    );
}

/// The window's commands that take a channel: `watch_session`, the one that streams a
/// terminal to a pane.
const TAKES_A_CHANNEL: &[&str] = &[
    "watch_session",
    "get_project",
    "create_project_on_forge",
    "publish_project",
];
