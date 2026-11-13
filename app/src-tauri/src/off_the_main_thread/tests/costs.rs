// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What each command the window asks costs the thread that asked (#1949's inventory), printed.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::test::{MockRuntime, mock_builder};

use super::tests::ask;
use crate::lifecycle::WINDOW;
use crate::planes::{PlaneId, Planes};

/// A project whose sandbox is on, with a local settings file beside its manifest (which is
/// what makes `sandbox_state` ask git), a persona, a session record and a usage file.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\nhosts = [\"example.com\", \"example.org\"]\n",
    )
    .expect("charter.toml");
    std::fs::write(
        purlis_core::names::local_settings(root),
        "[sandbox]\nhosts = [\"example.net\"]\n",
    )
    .expect("the local settings");
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("alpha");
    std::fs::write(root.join("workspaces/alpha/workspace.md"), "# alpha\n").expect("its md");
    std::fs::create_dir_all(root.join("personas/scribe")).expect("a persona");
    std::fs::write(
        root.join("personas/scribe/persona.md"),
        "---\nname: scribe\ndelegate_when: writing\n---\n# scribe\n",
    )
    .expect("its md");
    std::fs::create_dir_all(root.join("sessions")).expect("sessions/");
    std::fs::write(
        root.join("sessions/20261002-090000-record.md"),
        "# Record\n\n## Goal\n\nOne.\n\n## Done\n\nIt.\n",
    )
    .expect("a record");
    dir
}

fn app_on(root: &Path) -> (tauri::App<MockRuntime>, PlaneId) {
    let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None);
    let plane = planes.open(root);
    let app = mock_builder()
        .manage(planes)
        .invoke_handler(tauri::generate_handler![
            crate::sandboxing::sandbox_state,
            crate::sandboxing::persona_grants_held,
            crate::usage::chat_usage,
            crate::network::sandbox_network,
            crate::network::chat_network,
            crate::network::sandbox_kept_blocked,
            crate::dispatchgrants::dispatch_grants_needed,
            crate::dispatchgrants::dispatch_standing,
            crate::dispatchaway::dispatch_away,
            crate::sandboxing::grantable_folders,
            crate::harnessdecls::project_harness_declarations,
            crate::readonly::project_read_only,
            crate::plane_pins,
            crate::pin::plane_pin,
            crate::session_record,
            crate::worklinks::chat_work_item,
            crate::taskchanges::tasks_sharing_a_folder,
            crate::taskchanges::task_folder_shared,
            crate::opened_chats,
            crate::chats_that_would_not_start,
            crate::asking::asks_waiting,
            crate::asking::pending_asks,
            crate::chat_states,
            crate::chat_doings,
            crate::chat_skill_runs,
            crate::dispatched::task_question,
            crate::handoff::persona_chats_of,
            crate::stopping::task_ending,
            crate::stopping::all_tasks_ending,
            crate::vaultroute::vault_refusals,
            crate::activity::activity_chat,
            crate::smartclose::smart_closing,
            crate::smartclose::smart_close_offer,
            crate::stopping::stopping_chats,
            crate::sandboxing::owed_restarts,
            crate::views::reopened_views,
            crate::layout::reopened_layout,
            crate::views::reopened_focus,
            crate::chats_plane_updated,
            crate::open_planes,
            crate::title_bar_room,
            crate::killswitch::agents_stopped,
            crate::settings::check_sandbox_host,
            crate::sandboxing::sandbox_block_report,
            crate::updates::update_channel,
            crate::createproject::new_project_folder,
            crate::getproject::project_url_folder,
            crate::forgeaccounts::forge_token_help,
        ])
        .build(tauri_context!(test = true))
        .expect("the app builds");
    tauri::WebviewWindowBuilder::new(&app, WINDOW, tauri::WebviewUrl::default())
        .build()
        .expect("the main window");
    (app, plane)
}

/// What each read the window makes is asked with, on the project `plane`. A chat-keyed one
/// names chat 1, which is not open: it measures what the command reads before it looks for
/// the chat, which for most is the project's files and for some is nothing.
fn the_reads(plane: &PlaneId) -> Vec<(&'static str, Value)> {
    let p = json!({ "plane": plane });
    let chat = json!({ "plane": plane, "session": 1 });
    vec![
        ("sandbox_state", p.clone()),
        ("persona_grants_held", chat.clone()),
        ("chat_usage", chat.clone()),
        ("sandbox_network", p.clone()),
        ("chat_network", chat.clone()),
        ("sandbox_kept_blocked", p.clone()),
        ("dispatch_grants_needed", chat.clone()),
        ("dispatch_standing", p.clone()),
        ("dispatch_away", p.clone()),
        ("grantable_folders", p.clone()),
        ("project_harness_declarations", p.clone()),
        ("project_read_only", p.clone()),
        ("plane_pins", p.clone()),
        ("plane_pin", p.clone()),
        (
            "session_record",
            json!({ "plane": plane, "path": "sessions/20261002-090000-record.md" }),
        ),
        ("chat_work_item", chat.clone()),
        ("tasks_sharing_a_folder", chat.clone()),
        (
            "task_folder_shared",
            json!({ "plane": plane, "session": 1, "place": "alpha" }),
        ),
        ("opened_chats", p.clone()),
        ("chats_that_would_not_start", p.clone()),
        ("asks_waiting", p.clone()),
        ("pending_asks", p.clone()),
        ("chat_states", p.clone()),
        ("chat_doings", p.clone()),
        ("chat_skill_runs", p.clone()),
        ("task_question", chat.clone()),
        ("persona_chats_of", chat.clone()),
        ("task_ending", chat.clone()),
        ("all_tasks_ending", chat.clone()),
        ("vault_refusals", chat.clone()),
        ("activity_chat", json!({ "plane": plane, "key": "alpha" })),
        ("smart_closing", p.clone()),
        ("smart_close_offer", chat.clone()),
        ("stopping_chats", p.clone()),
        ("owed_restarts", p.clone()),
        ("reopened_views", p.clone()),
        ("reopened_layout", p.clone()),
        ("reopened_focus", p.clone()),
        ("chats_plane_updated", p),
        ("open_planes", json!({})),
        ("title_bar_room", json!({})),
        ("agents_stopped", json!({})),
        ("check_sandbox_host", json!({ "host": "example.com" })),
        (
            "sandbox_block_report",
            json!({ "operation": "network-outbound", "kind": "host", "harness": null }),
        ),
        ("update_channel", json!({})),
        (
            "new_project_folder",
            json!({ "name": "thing", "parent": null }),
        ),
        (
            "project_url_folder",
            json!({ "url": "https://example.com/a/thing.git", "parent": null }),
        ),
        (
            "forge_token_help",
            json!({ "kind": "github", "host": "github.com" }),
        ),
    ]
}

/// The median and the worst of `times`.
fn spread(mut times: Vec<Duration>) -> (Duration, Duration) {
    times.sort();
    (times[times.len() / 2], times[times.len() - 1])
}

/// How long each command holds the thread that asked, over many asks, ranked by the median:
/// `cargo test -p purlis-app costs -- --ignored --nocapture`. Each row is how long the asking
/// thread could do nothing else; for a command that answers off it, that is the hand-over.
#[test]
#[ignore = "a measurement, printed; run it by hand"]
fn what_each_command_holds_the_thread_that_asked_for() {
    const ASKS: usize = 200;
    let dir = a_project();
    let (app, plane) = app_on(dir.path());
    type Row<'a> = (&'a str, (Duration, Duration), (Duration, Duration), bool);
    let mut rows: Vec<Row<'_>> = the_reads(&plane)
        .into_iter()
        .map(|(command, args)| {
            // Once to warm what the first read of each file costs, then measured.
            let first = ask(&app, command, args.clone());
            let refused = first.answer.is_err();
            let (held, answered): (Vec<Duration>, Vec<Duration>) = (0..ASKS)
                .map(|_| {
                    let started = std::time::Instant::now();
                    let asked = ask(&app, command, args.clone());
                    (asked.held_for, started.elapsed())
                })
                .unzip();
            (command, spread(answered), spread(held), refused)
        })
        .collect();
    // Ranked by what the command costs, wherever it runs: from the ask to the answer.
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.0));
    println!(
        "| command | answered after: median | worst of {ASKS} | asking thread held: median \
         | worst | refused here |"
    );
    println!("| --- | --- | --- | --- | --- | --- |");
    for (command, (median, worst), (held, held_worst), refused) in rows {
        println!(
            "| `{command}` | {median:?} | {worst:?} | {held:?} | {held_worst:?} | {refused} |"
        );
    }

    // What one git process costs before it does anything: the least a command that runs one
    // holds the thread that asked, where the project is a repository (this sandbox cannot make
    // one, so `sandbox_state` above never reaches git).
    let spawns: Vec<Duration> = (0..20)
        .map(|_| {
            let started = std::time::Instant::now();
            purlis_core::forklock::output(std::process::Command::new("git").arg("--version"))
                .expect("git runs");
            started.elapsed()
        })
        .collect();
    let (median, worst) = spread(spawns);
    println!("one `git --version`: median {median:?}, worst {worst:?}");

    // `chat_usage` for an open chat: its usage file, a ring of sixteen rows.
    let sessions = purlis_core::usage::sessions_dir(dir.path());
    std::fs::create_dir_all(&sessions).expect("its sessions");
    let rows: String = (0..16)
        .map(|n| format!("{},{},90,{}\n", 1000 * n, 100 * n, n))
        .collect();
    std::fs::write(sessions.join("a-conversation.usage"), rows).expect("a usage file");
    let reads: Vec<Duration> = (0..ASKS)
        .map(|_| {
            let started = std::time::Instant::now();
            let _ = crate::usage::of(dir.path(), "a-conversation");
            started.elapsed()
        })
        .collect();
    let (median, worst) = spread(reads);
    println!("chat_usage's read of an open chat's usage file: median {median:?}, worst {worst:?}");

    // What a chat's tab, order, pin, focus or name costs as it is written down: the project's
    // reopen record, which those synchronous commands write each time (they stay synchronous
    // for their order, so this is what they hold the window for, less the change itself).
    let record = purlis_core::reopen::Record::default();
    let writes: Vec<Duration> = (0..ASKS)
        .map(|_| {
            let started = std::time::Instant::now();
            purlis_core::reopen::write(dir.path(), &record).expect("the record is written");
            started.elapsed()
        })
        .collect();
    let (median, worst) = spread(writes);
    println!("the reopen record written once: median {median:?}, worst {worst:?}");

    // And this machine's store, which `window_holds_planes` writes the window's tabs into on
    // every switch.
    let config = tempfile::tempdir().expect("a config home");
    let writes: Vec<Duration> = (0..ASKS)
        .map(|_| {
            let started = std::time::Instant::now();
            purlis_core::machine::update(config.path(), |store| store.windows = Vec::new())
                .expect("the store is written");
            started.elapsed()
        })
        .collect();
    let (median, worst) = spread(writes);
    println!("this machine's store written once: median {median:?}, worst {worst:?}");
}

/// Asks `command` as the window would and comes back at once, with where the answer will land:
/// [`ask`] without the wait, so a test can do something else while the command works.
fn ask_and_go_on(
    app: &tauri::App<MockRuntime>,
    command: &str,
    args: Value,
) -> (Duration, std::sync::mpsc::Receiver<bool>) {
    use tauri::Manager as _;
    use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse};
    use tauri::test::INVOKE_KEY;
    use tauri::webview::InvokeRequest;

    let window = app
        .get_webview_window(WINDOW)
        .expect("the main window is open");
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let asked_at = std::time::Instant::now();
    window.as_ref().clone().on_message(
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().expect("a URL"),
            body: InvokeBody::Json(args),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        },
        Box::new(move |_, _, response, _, _| {
            let _ = tx.send(matches!(response, InvokeResponse::Ok(_)));
        }),
    );
    (asked_at.elapsed(), rx)
}

#[test]
fn the_window_is_answered_while_the_reads_a_switch_makes_are_still_working() {
    // #1949: a switch asks `sandbox_state` of a project's notices and `persona_grants_held` of
    // every pane on screen, forty here. Each is held at the start of its work, as a slow git
    // would hold it; the thread that asked is back at once from every ask, a command of the
    // window's own answers meanwhile, and every held one answers once let go.
    let dir = a_project();
    let (app, plane) = app_on(dir.path());
    let held = crate::off_the_main_thread::gate::close(crate::sandboxing::READING_SANDBOX);

    let mut pending = Vec::new();
    for _ in 0..5 {
        pending.push(ask_and_go_on(
            &app,
            "sandbox_state",
            json!({ "plane": plane }),
        ));
    }
    for session in 1..=40 {
        pending.push(ask_and_go_on(
            &app,
            "persona_grants_held",
            json!({ "plane": plane, "session": session }),
        ));
    }
    for (held_for, _) in &pending {
        assert!(
            *held_for < Duration::from_secs(1),
            "the thread that asked was held for {held_for:?}"
        );
    }

    // Meanwhile: the window asks something of its own, and is answered.
    let meanwhile = ask(&app, "open_planes", json!({}));
    assert_eq!(meanwhile.answer, Ok(json!([plane])));
    // And none of the held reads has answered: what was answered did not wait for them.
    for (_, answer) in &pending {
        assert!(answer.try_recv().is_err(), "a held read answered");
    }

    drop(held);
    for (_, answer) in pending {
        assert!(
            answer
                .recv_timeout(Duration::from_secs(30))
                .expect("it answered once let go"),
            "it answered with a refusal"
        );
    }
}

#[test]
fn the_window_is_answered_while_the_asks_waiting_are_still_being_read() {
    // #5582: `asks_waiting` reads the project's files (its locks, sign-ins, removals and, since
    // #5579, each waiting persona's tools), and the window asks it on every burst of a chat's
    // moves. Held at the start of its reads, the thread that asked is back at once, the window
    // is answered meanwhile, and the list answers once let go.
    let dir = a_project();
    let (app, plane) = app_on(dir.path());
    let held = crate::off_the_main_thread::gate::close(crate::asking::READING_ASKS);

    let (held_for, answer) = ask_and_go_on(&app, "asks_waiting", json!({ "plane": plane }));
    assert!(
        held_for < Duration::from_secs(1),
        "the thread that asked was held for {held_for:?}"
    );
    let meanwhile = ask(&app, "open_planes", json!({}));
    assert_eq!(meanwhile.answer, Ok(json!([plane])));
    assert!(answer.try_recv().is_err(), "the held read answered");

    drop(held);
    assert!(
        answer
            .recv_timeout(Duration::from_secs(30))
            .expect("it answered once let go"),
        "it answered with a refusal"
    );
}

/// **The synchronous commands, each with why it may stay so** (#1949). Every other command
/// answers off the window's thread. A command is synchronous only when it touches no file, no
/// process and no lock another purlis holds (`IN_MEMORY`), or when the window relies on the
/// order its asks land in, which Tauri keeps only for synchronous commands (the rest).
const SYNCHRONOUS: &[(&str, &[&str])] = &[
    (
        // What the app holds in memory, or a word checked as written: well under a millisecond.
        "IN_MEMORY",
        &[
            "about_charter",
            // An artifact's link followed as a press would, and what the run opened in place
            // of the browser (ADR 0122, the `e2e` build only): the served page's snapshot and
            // the opened list, in memory; a link to a place is read off this thread.
            "artifact_link_follow",
            "artifact_links_opened",
            // Copy diagnostics' facts (#4774): the chats held in memory, and one uname(2).
            "diagnostics_here",
            "activity_chat",
            "pending_asks",
            "forget_sandbox_block",
            "keep_dispatch_blocked",
            "put_away_refused_vault",
            "keep_asked_workspace",
            "dismiss_sign_in",
            "dismiss_sandbox_only",
            "keep_sandbox_block_for_tasks",
            "choose_sandbox_block_target",
            "take_up_sandbox_block",
            "task_question",
            "persona_chats_of",
            "chats_that_would_not_start",
            "skill_move_needs_grant",
            "chat_states",
            "chat_doings",
            // The skill run open in each chat (SK-3): in memory.
            "chat_skill_runs",
            "running_sessions",
            "owed_restarts",
            // A project's chat moves, and putting one away (#2048): in memory.
            "chat_moves",
            "put_away_chat_move",
            // A move waiting on a sign-in prepared again (#3477): in memory, the preparing on its
            // own thread.
            "chat_move_signed_in",
            // A command confirmation's answer (`poolsignin_confirm`): in memory.
            "answer_pool_sign_in",
            "smart_closing",
            "task_ending",
            "all_tasks_ending",
            "stopping_chats",
            "vault_refusals",
            // A chat's failed MCP server starts, read and put away (qw381): in memory.
            "mcp_failures",
            "put_away_mcp_failure",
            // A chat lost with its session host, read and put away (#2007): in memory.
            "chat_lost_with_the_host",
            "put_away_lost_chat",
            // Which chats are reconnecting to their runner (#2496): in memory.
            "chats_reconnecting",
            // A task's fresh runners kept (#4569): in memory.
            "fresh_runners_kept",
            // Whether the session host is failing, and Try again (#2007): in memory, and a
            // wake of the supervisor's thread.
            "session_host_failing",
            "start_the_session_host_again",
            // A task's quiet note, read and put away (#1981): in memory.
            "task_quiet",
            "put_away_task_quiet",
            // What the core noticed of each chat, and It's fine (#6014): in memory.
            "chats_noticed",
            "stuck_fine",
            "reopened_views",
            "reopened_layout",
            "reopened_focus",
            "open_planes",
            "agents_stopped",
            "plane_at_launch",
            "session_bus",
            "vaults_to_move",
            "projects_handed",
            // What a tab window was made to draw (#4768): `Showing`'s, in memory.
            "tab_window_handed",
            "charter_windows",
            "title_bar_room",
            "first_frame",
            "extension_programs_run",
            "plane_fetch",
            "check_sandbox_host",
            "sandbox_block_report",
            "forge_token_help",
            "new_project_folder",
            "project_url_folder",
            "find_files_end",
            "search_files_end",
            "cancel_create_project",
            "cancel_get_project",
            "forge_device_sign_in_cancel",
            "forge_pkce_sign_in_cancel",
            "agent_identity_create_cancel",
            "pool_sign_in_cancel",
            "pool_setup_try_cancel",
        ],
    ),
    (
        // Reads this machine's store from the disk, a small file, once as Settings or the
        // updater is drawn. Its answer is a word with no error to carry a hand-off that did not
        // finish, so it moves off the window's thread with a change of its answer's type.
        "THIS_MACHINE_ONCE",
        &["update_channel"],
    ),
    (
        // A pane's keys, paste, mouse report, resize and its view's close (#4734, ADR 0108):
        // each joins the back of its chat's lane in the order the window asked, which Tauri
        // keeps only for synchronous commands, and answers at once. The lane's worker waits on
        // the host; the view's open is `async` and takes its turn behind them (#891).
        "QUEUED",
        &[
            "send_input",
            "send_input_bytes",
            "resize_session",
            "unwatch_session",
            // What a pane drew (#4733): in memory, answered at once, and after the writes it reports on.
            "session_drawn",
        ],
    ),
    (
        // What else lands in a chat, in the order asked: a smart close's prompt before its
        // cancel (#1007), a stop, a close before the resume that takes the chat's place, and
        // the fixed words a Keep blocked tells a chat. What each types takes its turn in the
        // chat's lane, behind what its pane queued, but waits for the lane no longer than
        // `inputlanes::A_TURN_WAITS_AT_MOST` and is refused past it; then it waits for the host
        // as before (ADR 0108), except a close: the host's answer to it changes nothing here, so
        // it is waited for no longer than `hostedsessions::A_CLOSE_WAITS_AT_MOST` on this
        // thread, and heard off it after (#6227). `async` would lose the order these keep.
        "INPUT_ORDER",
        &[
            "type_installer",
            "type_profile_sign_in",
            "reference_into_chat",
            "references_into_chat",
            "use_skill",
            "skill_draft_redo_in",
            "answer_ask",
            "smart_close_offer",
            "smart_close",
            "cancel_smart_close",
            "close_session",
            "stop_chat",
            // A paused chat let go on or stopped (#6015): in order with what is typed to it.
            "answer_pause",
            "ask_chat_restart",
            "keep_sandbox_block",
        ],
    ),
    (
        // The window's tabs, focus and front chat, each change applied in the order it was
        // sent: two tab moves answered out of order would leave the wrong one in place. The
        // change is made in memory here; the project's reopen record and this machine's store
        // it lands in are written behind the window (`planes::WrittenBehind`).
        "TAB_ORDER",
        &[
            "window_layout",
            "open_chat_tab",
            "close_chat_tab",
            "chat_order",
            "rename_chat",
            "window_views",
            "window_focus",
            // A pane of a window of a project with tab windows was used (#5108): noted in
            // memory, in the order the window's focus moved.
            "pane_used",
            "ignore_needs_you",
            "task_failure_seen",
            "forget_chat_that_did_not_start",
            "window_holds_planes",
            "inbox_shown",
            // Quiet (#6017): read, "Quiet now" turned on or off, or a pane typed in: in memory.
            "quiet_now",
            "set_quiet_now",
            "quiet_typed",
            "workspace_focused",
        ],
    ),
    (
        // The app's own window and process: shown, hidden, quit, restarted, or the kill
        // switch thrown back, in the order pressed. The update's check and install hand their
        // work to a task of their own and answer at once.
        "THE_APP",
        &[
            "ask_to_quit",
            "quit",
            "quit_cancelled",
            "hide_window",
            "window_showing",
            // The window's own native appearance and colour, set and read back on the window
            // the call came from (#2328): a property set on the window, as hiding it is.
            "draw_native_window",
            "native_window_drawn",
            "show_window_holding",
            // A press on a chat in another window of the project (#4768): the core's layout
            // read in memory, that window brought forward and told, as `show_window_holding`.
            "press_elsewhere",
            "restart_on_the_session_bus",
            "restart_to_update",
            "restart_to_update_choosing",
            "check_for_update",
            "install_update",
            "rearm_agents",
        ],
    ),
];

/// Every `#[tauri::command]` in this crate that is a plain `fn`, by name.
fn synchronous_commands() -> Vec<String> {
    fn walk(dir: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("the crate's sources") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, found);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file");
            let lines: Vec<&str> = text.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                if !line.trim_start().starts_with("#[tauri::command") {
                    continue;
                }
                let Some(signature) = lines[at + 1..]
                    .iter()
                    .take(8)
                    .find(|line| line.contains("fn "))
                else {
                    continue;
                };
                let before = signature.split("fn ").next().unwrap_or_default();
                if before.contains("async") {
                    continue;
                }
                let name: String = signature
                    .split("fn ")
                    .nth(1)
                    .unwrap_or_default()
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                found.push(name);
            }
        }
    }
    let mut found = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    found.sort();
    found
}

#[test]
fn a_command_is_synchronous_only_where_the_list_says_why() {
    // #1949: a synchronous command runs on the window's thread, which draws nothing while it
    // works. A new one that is not on the list fails here: make it `async` and do its work in
    // `crate::off_the_window`, or put it on the list under the reason it may stay.
    let found = synchronous_commands();
    let mut listed: Vec<String> = SYNCHRONOUS
        .iter()
        .flat_map(|(_, names)| names.iter().map(|name| (*name).to_owned()))
        .collect();
    listed.sort();
    let unlisted: Vec<&String> = found.iter().filter(|name| !listed.contains(name)).collect();
    assert!(
        unlisted.is_empty(),
        "synchronous, so answered on the window's thread, with no reason listed: {unlisted:?}"
    );
    // And the list holds only what is so: one made `async` comes off it.
    let stale: Vec<&String> = listed.iter().filter(|name| !found.contains(name)).collect();
    assert!(stale.is_empty(), "listed, but not synchronous: {stale:?}");
    let mut once = listed.clone();
    once.dedup();
    assert_eq!(once, listed, "a command listed twice");
}

/// A project with its manifest and nothing else: a chat starts in it without a sandbox.
fn a_bare_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    dir
}

/// The app on Tauri's mock runtime with a pane's terminal commands, a project whose chats run
/// in `host`, and one chat started in it: the project and the chat's number.
fn app_typing_into(
    host: &crate::host::pretend::Pretend,
    root: &Path,
) -> (tauri::App<MockRuntime>, PlaneId, u32) {
    let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None)
        .running_sessions_on(std::sync::Arc::new({
            let host = host.clone();
            move |_, _: &Path| Box::new(host.clone())
        }));
    app_typing_on(planes, root)
}

/// [`app_typing_into`], on `planes` as a test made it.
fn app_typing_on(planes: Planes, root: &Path) -> (tauri::App<MockRuntime>, PlaneId, u32) {
    let plane = planes.open(root);
    let chat = purlis_core::reopen::Chat {
        program: "/nowhere/a-harness".to_owned(),
        name: "one".to_owned(),
        ..Default::default()
    };
    let session = planes
        .held(&plane)
        .expect("held")
        .chats()
        .start(
            &chat,
            purlis_core::engine::Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("the host starts it");
    let app = mock_builder()
        .manage(planes)
        .invoke_handler(tauri::generate_handler![
            crate::send_input,
            crate::send_input_bytes,
            crate::resize_session,
            crate::watch_session,
            crate::unwatch_session,
            crate::firstrun::type_installer,
        ])
        .build(tauri_context!(test = true))
        .expect("the app builds");
    tauri::WebviewWindowBuilder::new(&app, WINDOW, tauri::WebviewUrl::default())
        .build()
        .expect("the main window");
    (app, plane, session)
}

/// What a pane sends as a person pastes, types and its terminal is resized, in that order: a
/// paste, a key, a resize, a mouse report's bytes, a resize, a key.
fn a_pane_sending(plane: &PlaneId, session: u32) -> Vec<(&'static str, Value)> {
    let to = |more: Value| {
        let mut args = json!({ "plane": plane, "session": session });
        args.as_object_mut()
            .expect("an object")
            .extend(more.as_object().expect("an object").clone());
        args
    };
    vec![
        (
            "send_input",
            to(json!({ "text": "\u{1b}[200~pasted\u{1b}[201~" })),
        ),
        ("send_input", to(json!({ "text": "x" }))),
        ("resize_session", to(json!({ "columns": 100, "rows": 30 }))),
        (
            "send_input_bytes",
            to(json!({ "bytes": [0x1b, 0x5b, 0x4d] })),
        ),
        ("resize_session", to(json!({ "columns": 90, "rows": 20 }))),
        ("send_input", to(json!({ "text": "y" }))),
    ]
}

#[test]
fn no_input_resize_or_view_waits_on_the_host_on_the_windows_thread() {
    // #4734: a pane's input, its resizes and its view's open and close each went to the session
    // host on the window's thread and waited for its answer, up to 30 s on a host that does not
    // answer. Here the host takes a second for each: the window's thread is back at once from
    // every one, and the view's open answers off it.
    let dir = a_bare_project();
    let host = crate::host::pretend::Pretend::default();
    let (app, plane, session) = app_typing_into(&host, dir.path());
    host.terminal_takes(Duration::from_secs(1));

    for (command, args) in a_pane_sending(&plane, session) {
        let asked = ask(&app, command, args);
        assert!(
            asked.held_for < Duration::from_millis(250),
            "`{command}` held the window's thread for {:?} on a host that takes a second",
            asked.held_for
        );
        assert_eq!(asked.answer, Ok(Value::Null), "`{command}` was refused");
    }
    let (held_for, opened) = ask_and_go_on(
        &app,
        "watch_session",
        json!({ "plane": plane, "session": session, "output": "__CHANNEL__:1" }),
    );
    assert!(
        held_for < Duration::from_millis(250),
        "`watch_session` held the window's thread for {held_for:?}"
    );
    let unwatch = ask(
        &app,
        "unwatch_session",
        json!({ "plane": plane, "session": session, "view": 1 }),
    );
    assert!(
        unwatch.held_for < Duration::from_millis(250),
        "`unwatch_session` held the window's thread for {:?}",
        unwatch.held_for
    );
    assert!(
        opened
            .recv_timeout(Duration::from_secs(30))
            .expect("the view answered once the host did"),
        "the view was refused"
    );
}

#[test]
fn a_chats_paste_keys_resizes_and_view_reach_its_host_in_the_order_the_pane_sent_them() {
    // #4734 amends #891's rule: what a pane sends no longer waits on the window's thread, so
    // its order is kept by the chat's own queue. A host that takes a while for each shows any
    // send that overtook another. The view opens after the resizes before it, and closes after.
    let dir = a_bare_project();
    let host = crate::host::pretend::Pretend::default();
    let (app, plane, session) = app_typing_into(&host, dir.path());
    host.terminal_takes(Duration::from_millis(40));

    for (command, args) in a_pane_sending(&plane, session) {
        assert_eq!(ask(&app, command, args).answer, Ok(Value::Null));
    }
    let (_, opened) = ask_and_go_on(
        &app,
        "watch_session",
        json!({ "plane": plane, "session": session, "output": "__CHANNEL__:1" }),
    );
    // As the pane does: the view is closed by the id its open answered with.
    assert!(
        opened
            .recv_timeout(Duration::from_secs(30))
            .expect("the view answered"),
        "the view was refused"
    );
    let closed = ask(
        &app,
        "unwatch_session",
        json!({ "plane": plane, "session": session, "view": 1 }),
    );
    assert_eq!(closed.answer, Ok(Value::Null));

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while host.terminal().len() < 8 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        host.terminal(),
        vec![
            format!("typed {session} \u{1b}[200~pasted\u{1b}[201~"),
            format!("typed {session} x"),
            format!("resized {session} 100x30"),
            format!("typed {session} \u{1b}[M"),
            format!("resized {session} 90x20"),
            format!("typed {session} y"),
            format!("watched {session}"),
            format!("unwatched {session} 1"),
        ]
    );
}

#[test]
fn a_chat_whose_runner_is_reconnecting_is_sent_nothing_and_its_pane_is_told_at_once() {
    // #4734: a link that is down fails fast to the chat's pane, never by holding the window's
    // thread or the chat's lane for the host's 30 s. The host here would take a second for each.
    let dir = a_bare_project();
    let host = crate::host::pretend::Pretend::default();
    let (told, heard) = std::sync::mpsc::channel();
    let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None)
        .running_sessions_on(std::sync::Arc::new({
            let host = host.clone();
            move |_, _: &Path| Box::new(host.clone())
        }))
        .telling_input_refused(std::sync::Arc::new(move |refused| {
            let _ = told.send(refused);
        }));
    let (app, plane, session) = app_typing_on(planes, dir.path());
    host.terminal_takes(Duration::from_secs(1));
    host.link_down(&[session]);
    let asked_at = std::time::Instant::now();

    for (command, args) in a_pane_sending(&plane, session) {
        let asked = ask(&app, command, args);
        assert!(
            asked.held_for < Duration::from_millis(250),
            "`{command}` held the window's thread for {:?}",
            asked.held_for
        );
        assert_eq!(asked.answer, Ok(Value::Null), "`{command}` was refused");
    }

    let refused = heard
        .recv_timeout(Duration::from_secs(5))
        .expect("the chat's pane is told");
    assert!(
        asked_at.elapsed() < Duration::from_millis(900),
        "told after {:?}, as long as the host would have taken",
        asked_at.elapsed()
    );
    assert_eq!(
        refused,
        crate::inputlanes::InputRefused {
            plane: plane.clone(),
            session,
            why: crate::inputlanes::RECONNECTING.to_owned(),
        }
    );
    assert!(
        host.terminal().is_empty(),
        "the host was asked: {:?}",
        host.terminal()
    );
}

#[test]
fn a_views_close_behind_a_refused_key_still_reaches_the_host() {
    // Review of #4821: a key the host refused dropped everything queued behind it, the pane's
    // view's close too, and a view never closed is one the host keeps open, and paced, for the
    // rest of the session (#4810). Only what the person typed is dropped.
    let dir = a_bare_project();
    let host = crate::host::pretend::Pretend::default();
    let (app, plane, session) = app_typing_into(&host, dir.path());
    host.terminal_takes(Duration::from_secs(1));
    let chat = json!({ "plane": plane, "session": session });
    let with = |more: Value| {
        let mut args = chat.clone();
        args.as_object_mut()
            .expect("an object")
            .extend(more.as_object().expect("an object").clone());
        args
    };

    // The first key holds the lane while the link goes down: the next key is refused, and the
    // view's close is queued behind it.
    assert_eq!(
        ask(&app, "send_input", with(json!({ "text": "a" }))).answer,
        Ok(Value::Null)
    );
    // Its worker has asked the host by now, which takes a second over it.
    std::thread::sleep(Duration::from_millis(200));
    host.link_down(&[session]);
    for (command, args) in [
        ("send_input", with(json!({ "text": "b" }))),
        ("send_input", with(json!({ "text": "c" }))),
        ("unwatch_session", with(json!({ "view": 1 }))),
    ] {
        assert_eq!(ask(&app, command, args).answer, Ok(Value::Null));
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while host.terminal().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        host.terminal(),
        vec![
            format!("typed {session} a"),
            format!("unwatched {session} 1")
        ]
    );
}

#[test]
fn what_types_into_a_chat_on_the_windows_thread_waits_on_its_lane_no_longer_than_its_deadline() {
    // Review of #4821: a command that types into a chat (a stop's line, a smart close's prompt,
    // an installer) takes its turn in the chat's lane on the window's thread. Behind a lane
    // stuck on a slow host it waited for all of it, then for its own answer, and the window
    // froze meanwhile. It waits for the lane no longer than `inputlanes::A_TURN_WAITS_AT_MOST`
    // and is refused, unsent, past it.
    let dir = a_bare_project();
    let host = crate::host::pretend::Pretend::default();
    let (app, plane, session) = app_typing_into(&host, dir.path());
    host.terminal_takes(Duration::from_secs(4));
    let chat = json!({ "plane": plane, "session": session });
    let mut key = chat.clone();
    key["text"] = json!("a");
    assert_eq!(ask(&app, "send_input", key).answer, Ok(Value::Null));

    let mut installer = chat;
    installer["harness"] = json!("claude");
    let typed = ask(&app, "type_installer", installer);

    assert!(
        typed.held_for < crate::inputlanes::A_TURN_WAITS_AT_MOST + Duration::from_millis(500),
        "`type_installer` held the window's thread for {:?} behind a stuck lane",
        typed.held_for
    );
    assert!(
        typed.answer.is_err(),
        "typed behind a lane that never emptied"
    );
}
