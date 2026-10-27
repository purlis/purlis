// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The UI RPC runs on the same link as the session protocol, for the app's window and its own
//! build only (FD-26, ADR 0068 §4 and §5). A window of another build, or a client of another
//! scope, is refused it, and keeps the session protocol.

use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Answer, Client, Command, Frame, Host, Peer, Refusal};
use purlis_session_protocol::ui::{self, Handler};
use serde_json::{Value, json};
use tokio::io::duplex;

mod common;
use common::HELD;

/// The app's commands, as the host answers them: `rename_chat` succeeds, `opened_chats` fails
/// with the command's own error, and every call is kept.
#[derive(Default, Clone)]
struct Commands {
    asked: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Handler for Commands {
    fn call(&self, method: String, args: Value) -> BoxFuture<'_, Result<Value, Value>> {
        Box::pin(async move {
            self.asked
                .lock()
                .unwrap()
                .push((method.clone(), args.clone()));
            match method.as_str() {
                "rename_chat" => Ok(json!({"renamed": args["name"]})),
                "pending_asks" => Ok(an_edit_waiting()),
                _ => Err(json!("that plane is not open")),
            }
        })
    }
}

/// What `pending_asks` answers with an edit waiting: its row, and the change it would write.
fn an_edit_waiting() -> Value {
    json!({
        "plane": 1,
        "asks": [{
            "session": 3,
            "ask": "a",
            "says": "Change src/lib.rs",
            "options": [
                {"id": "allow", "label": "Allow", "allows": true},
                {"id": "deny", "label": "Deny", "allows": false}
            ],
            "source": "permission",
            "chain": [],
            "answer": {"via": "hook"},
            "writes": {"kind": "whole", "path": "src/lib.rs", "changes": [{"says": "Writes the file", "before": "", "after": "new text", "hunks": []}]}
        }]
    })
}

struct Sessions;

impl Host for Sessions {
    async fn call(&self, _command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        Ok(Answer::Chats(Vec::new()))
    }
}

const BUILD: &str = "0.4.0+9055f7a";

async fn linked(scope: Scope, commands: Option<Commands>) -> Client {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = commands.map(|c| ui::Server::new(BUILD, ["rename_chat", "opened_chats"], c));
    tokio::spawn(session::serve(served.unwrap(), Sessions, ui));
    Client::new(client.unwrap()).0
}

#[tokio::test]
async fn the_window_of_the_same_build_calls_the_apps_commands_on_the_link() {
    let commands = Commands::default();
    let client = linked(Scope::LocalUi, Some(commands.clone())).await;
    let ui = client.ui(BUILD).await.unwrap();

    let renamed = ui
        .call("rename_chat", json!({"chat": 5, "name": "login"}))
        .await
        .unwrap();
    assert_eq!(renamed, Ok(json!({"renamed": "login"})));
    let failed = ui.call("opened_chats", json!({"plane": 2})).await.unwrap();
    assert_eq!(failed, Err(json!("that plane is not open")));
    assert_eq!(
        commands.asked.lock().unwrap().clone(),
        vec![
            (
                "rename_chat".to_owned(),
                json!({"chat": 5, "name": "login"})
            ),
            ("opened_chats".to_owned(), json!({"plane": 2})),
        ]
    );
    // And the session protocol, beside it on the same lane.
    assert_eq!(client.list().await.unwrap(), Vec::new());
}

#[tokio::test]
async fn a_method_the_host_does_not_serve_is_refused_before_it_is_asked() {
    let commands = Commands::default();
    let client = linked(Scope::LocalUi, Some(commands.clone())).await;
    let ui = client.ui(BUILD).await.unwrap();
    let refused = ui.call("vault_secret_reveal", json!({})).await.unwrap();
    assert!(refused.is_err());
    assert!(commands.asked.lock().unwrap().is_empty());
}

/// Issue 1916: a link is served a branch's folders to watch and never the plane root's. The
/// call is refused on the link itself, with the protocol's own sentence, and the app's command
/// is never asked.
#[tokio::test]
async fn a_folder_watch_that_names_the_plane_root_is_refused_before_the_command_is_asked() {
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(BUILD, ["files_watch", "rename_chat"], commands.clone());
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();
    let folder = |workspace: &str, repo: &str| json!({"plane": "/p", "workspace": workspace, "repo": repo, "piece": null, "folder": "docs"});

    for args in [
        json!({"folders": [folder("", "")]}),
        // One such folder among a branch's refuses the whole call.
        json!({"folders": [folder("alpha", "svc"), folder("", "")]}),
        json!({"folders": [{"plane": "/p", "folder": ""}]}),
        // A call that does not say its folders as a list names no branch.
        json!({}),
        json!([[folder("alpha", "svc")]]),
    ] {
        let refused = ui.call("files_watch", args.clone()).await.unwrap();
        assert_eq!(
            refused,
            Err(json!(ui::THE_PLANE_ROOT_IS_THE_WINDOWS)),
            "{args}"
        );
    }
    assert!(commands.asked.lock().unwrap().is_empty());

    // A branch's folders are the command's to answer, as before.
    let branch = json!({"folders": [folder("alpha", "svc")]});
    let answered = ui.call("files_watch", branch.clone()).await.unwrap();
    assert_eq!(answered, Err(json!("that plane is not open")));
    assert_eq!(
        commands.asked.lock().unwrap().clone(),
        vec![("files_watch".to_owned(), branch)]
    );
}

#[tokio::test]
async fn answering_an_ask_is_never_served_on_the_link_even_to_the_window() {
    // HP-6: `answer_ask` is the window's over Tauri's IPC alone, even when a host is built with
    // the app's whole command list.
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(
        BUILD,
        ["rename_chat", "answer_ask", "answer_task_question"],
        commands.clone(),
    );
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    let refused = ui
        .call(
            "answer_ask",
            json!({"plane": 1, "session": 3, "ask": "a", "option": "allow"}),
        )
        .await
        .unwrap();

    assert!(refused.is_err(), "{refused:?}");
    assert!(commands.asked.lock().unwrap().is_empty());
    assert!(ui::WINDOW_ONLY.contains(&"answer_ask"));

    // #1496: the person's answer to a question a task put to its asking chat reaches that
    // task marked as the person's own words, and is held to the same.
    let refused = ui
        .call(
            "answer_task_question",
            json!({"plane": 1, "session": 3, "question": "Which queue?", "text": "The second."}),
        )
        .await
        .unwrap();

    assert!(refused.is_err(), "{refused:?}");
    assert!(commands.asked.lock().unwrap().is_empty());
    assert!(ui::WINDOW_ONLY.contains(&"answer_task_question"));
    // Every command that ends, starts or restarts a chat on the person's word is the window's
    // too (#1488). None of these is served on a link.
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
        "relaunch",
        "restart_chat_without_sandbox",
        "restart_chat",
        "start_chat_fresh",
        "ask_chat_restart",
    ] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    // And the list is exactly that rule's, with Stop all tasks and its question (#1498), the
    // three reads of what chats said or were sent (#1494, #1495, #1496), the person's answer
    // to a task (#1496), the dispatch commands (spec #1483) and the person's "Got it" on the
    // presets Notice (#1385) and the hosts Notice (#1550): nothing else is kept from a link by it.
    assert_eq!(ui::STANDING_DISPATCH.len(), 26);
    assert!(ui::WINDOW_ONLY.contains(&"acknowledge_project_presets"));
    // And the person's acts named beside them: a task's merge and discard (#1511), the answer
    // to a chat's sandbox block and to several tasks' (#1538, #1508), and the Allow of a
    // persona's hosts on this machine (#1362), the delete of a task's merged branch whose
    // folder is gone (#1472), Allow on Settings' Blocked lately (#1662), and Keep blocked on a
    // sandbox host ask (#1690).
    for command in [
        "task_branch_merge",
        "dispatch_worktree_discard",
        "task_branch_delete",
        "allow_sandbox_block",
        "allow_sandbox_block_for_tasks",
        "keep_sandbox_block_for_tasks",
        // One chat's Keep blocked, which refuses a connection its proxy holds (#1666).
        "keep_sandbox_block",
        "allow_persona_hosts",
        // The answer to what a project declares it expects (ADR 0093): Allow, and Not now.
        "allow_project_expects",
        "decline_project_expects",
        "allow_blocked_host",
        "allow_sandbox_port",
        "forget_sandbox_block",
    ] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    assert!(ui::WINDOW_ONLY.contains(&"acknowledge_project_hosts"));
    // And the start of the person's editor on this machine's extension record (#1296).
    assert!(ui::WINDOW_ONLY.contains(&"open_extension_record"));
    // And every ask that waits on the person, from every source (#1690).
    assert!(ui::WINDOW_ONLY.contains(&"asks_waiting"));
    // And the Inbox's updates: what happened to the person's chats, and what they put away
    // (#1693).
    for command in [
        "inbox_updates",
        "note_inbox_updates",
        "settle_inbox_updates",
    ] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    // And several rows typed into a chat as one paste, held to the window until typing into a
    // chat has its ruling (#1151).
    assert!(ui::WINDOW_ONLY.contains(&"references_into_chat"));
    // And why a project is read-only to this purlis, said in its tab (#826).
    assert!(ui::WINDOW_ONLY.contains(&"project_read_only"));
    // And the first task's runs read back with their diff commands (#945).
    assert!(ui::WINDOW_ONLY.contains(&"first_task_started"));
    // And what the person kept blocked here, read for the Inbox (#1700).
    assert!(ui::WINDOW_ONLY.contains(&"sandbox_kept_blocked"));
    // And the project's harness declarations, with Open file on one (#1389).
    for command in ["project_harness_declarations", "open_harness_declaration"] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    // And Restart to update with what to put back already answered (#1078).
    assert!(ui::WINDOW_ONLY.contains(&"restart_to_update_choosing"));
    // And the developer tools' install, which starts a system install on this machine (#1813).
    assert!(ui::WINDOW_ONLY.contains(&"install_developer_tools"));
    // And signing out of a forge account, which deletes its token (#1814).
    assert!(ui::WINDOW_ONLY.contains(&"forge_sign_out"));
    // And the person's own clone by URL: its folder and its Cancel (#1815).
    assert!(ui::WINDOW_ONLY.contains(&"project_url_folder"));
    assert!(ui::WINDOW_ONLY.contains(&"cancel_get_project"));
    // And the list of this machine's forge accounts, which no link reads (#1816).
    assert!(ui::WINDOW_ONLY.contains(&"forge_accounts"));
    // And where the person is in the guided steps, read and kept (#1817).
    assert!(ui::WINDOW_ONLY.contains(&"onboarding_progress"));
    assert!(ui::WINDOW_ONLY.contains(&"keep_onboarding_progress"));
    // And creating a project on a forge: its owners, its folder and its Cancel (#1819).
    assert!(ui::WINDOW_ONLY.contains(&"forge_owners"));
    assert!(ui::WINDOW_ONLY.contains(&"new_project_folder"));
    assert!(ui::WINDOW_ONLY.contains(&"cancel_create_project"));
    // And the repos those accounts reach, listed as them (#1818).
    assert!(ui::WINDOW_ONLY.contains(&"forge_repo_list"));
    // And whether a folder is in a project, asked before offering to make it one (#1822).
    assert!(ui::WINDOW_ONLY.contains(&"folder_is_a_project"));
    // And an existing gh or glab login: which ones could be imported, and the import (#1376).
    assert!(ui::WINDOW_ONLY.contains(&"forge_cli_logins"));
    assert!(ui::WINDOW_ONLY.contains(&"forge_import_cli_login"));
    // And the browser sign-in by the device flow: its start, its wait and its cancel (#729).
    assert!(ui::WINDOW_ONLY.contains(&"forge_device_sign_in_start"));
    assert!(ui::WINDOW_ONLY.contains(&"forge_device_sign_in_wait"));
    assert!(ui::WINDOW_ONLY.contains(&"forge_device_sign_in_cancel"));
    assert!(ui::WINDOW_ONLY.contains(&"forge_pkce_sign_in"));
    assert!(ui::WINDOW_ONLY.contains(&"forge_pkce_sign_in_cancel"));
    // And whether a project lives only on this machine, beside its Publish (#1820).
    assert!(ui::WINDOW_ONLY.contains(&"project_lives_here"));
    // And the person's answers to the Saving view's question about a clone's remote (#1848,
    // #1852): each settles where automatic saves push with their sign-in.
    assert!(ui::WINDOW_ONLY.contains(&"confirm_remote"));
    assert!(ui::WINDOW_ONLY.contains(&"keep_recorded_remote"));
    // And filing a sandbox block's report, as the person with their sign-in (#806).
    assert!(ui::WINDOW_ONLY.contains(&"file_sandbox_block_report"));
    // And opening a branch's file in the person's editor, which opens a terminal editor in a
    // shell tab of its own (#1042): a shell started as the person.
    assert!(ui::WINDOW_ONLY.contains(&"open_in_your_editor"));
    assert!(ui::WINDOW_ONLY.contains(&"open_their_agents_md"));
    // And what the person chooses where a sandbox block named nothing (#1884): the list this
    // machine offers, the choice, and taking up a block a chat only read back.
    for command in [
        "sandbox_block_choices",
        "choose_sandbox_block_target",
        "take_up_sandbox_block",
    ] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    // And opening a file the project's save is blocked in, from the Saving view: the person's
    // editor, started on their press.
    assert!(ui::WINDOW_ONLY.contains(&"open_blocked_file"));
    // And Update from remote, which moves the project's working tree under its chats (qw348).
    assert!(ui::WINDOW_ONLY.contains(&"update_project"));
    // And merging a save's pull request past its failed or missing checks (ADR 0051, amended
    // 2026-10-13): the person's own press in the window, which no link and no chat sends.
    assert!(ui::WINDOW_ONLY.contains(&"merge_save_request"));
    assert!(ui::window_only("merge_save_request"));
    assert!(
        !ui::served_on_a_link("merge_save_request")
            && !ui::SERVED_ON_A_LINK.contains(&"merge_save_request"),
        "a link was served the person's merge"
    );
    // And the person's Keep on a chat's question whether to remove a workspace (qw356), which
    // no link and no chat sends. The removal has no command of the question's: it is the
    // window's own Delete workspace.
    assert!(ui::WINDOW_ONLY.contains(&"keep_asked_workspace"));
    assert!(!ui::window_only_commands().any(|one| one == "remove_asked_workspace"));
    // #1927: removal, grant, settings and credential commands are the window's alone. The
    // list is the source; one of each class is named here, so taking a class off the list
    // fails this test and not only a generated file's diff.
    for command in [
        "revoke_sandbox_grant",
        "workspace_remove",
        "allow_refused_vault",
        "save_project_settings",
        "install_extension",
        "reference_into_chat",
    ] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    for command in ["vault_secret_reveal", "vault_secret_set"] {
        assert!(ui::WINDOW_ONLY_CREDENTIALS.contains(&command), "{command}");
    }
}

#[tokio::test]
async fn every_command_that_is_the_windows_alone_is_refused_on_a_link_before_it_is_asked() {
    // #1542: the list says itself. A command is declared the window's alone in one place
    // (`ui::WINDOW_ONLY`, or `ui::WINDOW_ONLY_CREDENTIALS` for one that takes a credential),
    // and this holds every one of them, however many there are, to what the tests above hold
    // the ones they name: a host built with the app's whole command list serves none.
    let all: Vec<&str> = ui::window_only_commands().collect();
    let mut named_once = all.clone();
    named_once.sort_unstable();
    named_once.dedup();
    assert_eq!(named_once.len(), all.len(), "a command is listed twice");
    for command in ui::WINDOW_ONLY.iter().chain(ui::WINDOW_ONLY_CREDENTIALS) {
        assert!(all.contains(command), "{command}");
    }

    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(
        BUILD,
        all.iter()
            .copied()
            .chain(["rename_chat", "a_command_added_later"]),
        commands.clone(),
    );
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    for command in &all {
        assert!(ui::window_only(command), "{command}");
        let refused = ui.call(command, json!({"plane": 1})).await.unwrap();
        assert!(refused.is_err(), "{command}: {refused:?}");
    }
    // #1927: and a command on neither list, as one added to the app and not yet listed would
    // be, is refused the same way although the host was built with it: a link is served a
    // command only where the served list names it.
    assert!(!ui::served_on_a_link("a_command_added_later"));
    let unlisted = ui.call("a_command_added_later", json!({})).await.unwrap();
    assert!(unlisted.is_err(), "{unlisted:?}");
    assert!(
        commands.asked.lock().unwrap().is_empty(),
        "a command of the window's alone, or one no list names, reached the host's handler"
    );
    // And the one beside them that is not the window's alone is served.
    let renamed = ui
        .call("rename_chat", json!({"chat": 5, "name": "login"}))
        .await
        .unwrap();
    assert_eq!(renamed, Ok(json!({"renamed": "login"})));
}

#[tokio::test]
async fn no_standing_dispatch_grant_is_made_changed_or_taken_back_on_any_link() {
    // Spec #1483, train 64: every command that makes, widens, accepts, declines or takes back
    // a standing dispatch grant, every answer to a dispatch's question (Keep blocked too: it
    // is the person's answer), the reads of what stands and what waits, and the lists a
    // person answers from, are the window's over
    // Tauri's IPC alone, even when a host is built with every command. Named here one by one,
    // so taking one off the list fails this test and not only a generated file's diff.
    const STANDING: [&str; 26] = [
        "dispatch_grants_needed",
        "dispatch_standing",
        "dispatch_grants",
        "allow_dispatch",
        "allow_dispatch_anywhere",
        "keep_dispatch_blocked",
        "never_dispatch",
        "lift_dispatch_never",
        "allow_dispatch_to_any",
        "add_dispatch_grant",
        "revoke_dispatch_to_any",
        "revoke_dispatch_grant",
        "accept_project_dispatch",
        "decline_project_dispatch",
        "accept_project_dispatch_in",
        "decline_project_dispatch_in",
        "set_dispatch_workspace",
        "give_back_dispatch",
        "remove_dormant_dispatch",
        "dispatch_arrival",
        "answer_dispatch_arrival",
        "dispatch_gone_told",
        "dispatch_away",
        "allow_dispatch_away",
        "dismiss_dispatch_away",
        "never_dispatch_away",
    ];
    assert_eq!(ui::STANDING_DISPATCH, STANDING);
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(
        BUILD,
        STANDING.into_iter().chain(["rename_chat"]),
        commands.clone(),
    );
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    for command in STANDING {
        let refused = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            ui.call(
                command,
                json!({"plane": 1, "id": 1, "level": "project", "asking": "steward", "target": "devops"}),
            ),
        )
        .await
        .unwrap_or_else(|_| panic!("{command} was answered"))
        .unwrap();
        assert!(refused.is_err(), "{command}: {refused:?}");
    }
    assert!(commands.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_refusal_kept_while_nobody_was_there_is_read_and_answered_on_no_link() {
    // #1507: the list and its three answers (one a standing grant in one press) are the
    // window's over Tauri's IPC alone, even when a host is built with every command.
    const AWAY: [&str; 4] = [
        "dispatch_away",
        "allow_dispatch_away",
        "dismiss_dispatch_away",
        "never_dispatch_away",
    ];
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(
        BUILD,
        AWAY.into_iter().chain(["rename_chat"]),
        commands.clone(),
    );
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    for command in AWAY {
        let refused = ui
            .call(
                command,
                json!({"plane": 1, "asking": "steward", "target": "devops", "workspace": null}),
            )
            .await
            .unwrap();
        assert!(refused.is_err(), "{command}: {refused:?}");
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
    assert!(commands.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_task_s_brief_is_never_served_on_the_link_even_to_the_window() {
    // #1494: the brief a task was sent is read over Tauri's IPC alone, even when a host is
    // built with the app's whole command list.
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(BUILD, ["rename_chat", "task_brief"], commands.clone());
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    for of in [
        json!({"chat": 3}),
        json!({"dispatch": "01K6DISPATCH00000000000000"}),
    ] {
        let refused = ui
            .call("task_brief", json!({"plane": 1, "of": of}))
            .await
            .unwrap();
        assert!(refused.is_err(), "{refused:?}");
    }

    assert!(commands.asked.lock().unwrap().is_empty());
    assert!(ui::WINDOW_ONLY.contains(&"task_brief"));
}

#[tokio::test]
async fn what_a_session_and_its_tasks_said_is_never_served_on_the_link_even_to_the_window() {
    // A session's Activity (#1495) and the question a task is paused on (#1496) are words
    // chats wrote, read by a session the caller names: Tauri's IPC alone, as a brief is.
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(
        BUILD,
        ["rename_chat", "activity", "task_question"],
        commands.clone(),
    );
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    for method in ["activity", "task_question"] {
        let refused = ui
            .call(method, json!({"plane": 1, "session": 3}))
            .await
            .unwrap();
        assert!(refused.is_err(), "{method}: {refused:?}");
        assert!(ui::WINDOW_ONLY.contains(&method), "{method}");
    }
    assert!(commands.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_window_of_another_build_is_refused_the_ui_rpc_and_keeps_the_session_protocol() {
    let commands = Commands::default();
    let client = linked(Scope::LocalUi, Some(commands.clone())).await;
    let refused = client.ui("0.3.9+1111111").await.err().unwrap();
    assert!(
        refused.to_string().contains("private to one build"),
        "{refused}"
    );
    assert_eq!(client.list().await.unwrap(), Vec::new());
}

#[tokio::test]
async fn a_client_of_another_scope_is_refused_the_ui_rpc() {
    for scope in [
        Scope::Terminal,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
    ] {
        let commands = Commands::default();
        let client = linked(scope, Some(commands.clone())).await;
        let refused = client.ui(BUILD).await.err().unwrap();
        assert!(refused.to_string().contains("local-ui"), "{refused}");
        assert!(commands.asked.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn a_host_that_serves_no_ui_rpc_refuses_it() {
    let client = linked(Scope::LocalUi, None).await;
    assert!(client.ui(BUILD).await.is_err());
    assert_eq!(client.list().await.unwrap(), Vec::new());
}

/// A UI RPC call sent raw, as `scope`, with no hello first: what the host answers, and the
/// commands it may have asked.
async fn called_without_a_hello(scope: Scope) -> (Value, Commands) {
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(BUILD, ["rename_chat"], commands.clone());
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let mut link = client.unwrap();
    link.control()
        .send(bytes::Bytes::from(
            json!({"ui": {"call": {"id": 1, "method": "rename_chat", "args": {}}}}).to_string(),
        ))
        .await
        .unwrap();
    let answer: Value =
        serde_json::from_slice(&link.control().next().await.unwrap().unwrap()).unwrap();
    (answer, commands)
}

#[tokio::test]
async fn a_call_before_the_hello_is_refused() {
    let (answer, commands) = called_without_a_hello(Scope::LocalUi).await;
    assert_eq!(answer["ui"]["reply"]["re"], 1);
    assert!(answer["ui"]["reply"]["err"].is_string());
    assert!(commands.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_client_of_another_scope_that_skips_the_hello_is_refused_too() {
    for scope in [
        Scope::Terminal,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
    ] {
        let (answer, commands) = called_without_a_hello(scope).await;
        assert!(answer["ui"]["reply"]["err"].is_string(), "{scope:?}");
        assert!(answer["ui"]["reply"].get("ok").is_none(), "{scope:?}");
        assert!(commands.asked.lock().unwrap().is_empty(), "{scope:?}");
    }
}

#[tokio::test]
async fn a_refused_hello_closes_what_an_earlier_one_opened() {
    let commands = Commands::default();
    let client = linked(Scope::LocalUi, Some(commands.clone())).await;
    let ui = client.ui(BUILD).await.unwrap();
    assert!(client.ui("0.3.9+1111111").await.is_err());
    let refused = ui.call("rename_chat", json!({})).await.unwrap();
    assert!(refused.is_err());
    assert!(commands.asked.lock().unwrap().is_empty());
}

/// The frames the app's TypeScript client writes and reads (`app/src/uiRpcLink.test.ts` holds
/// the same lines), so the two ends agree on the wire.
#[test]
fn the_typescript_clients_frames_are_the_ones_the_host_reads() {
    let hello: Frame =
        serde_json::from_str(r#"{"ui":{"hello":{"build":"0.4.0+9055f7a"}}}"#).unwrap();
    assert_eq!(
        hello,
        Frame::Ui(ui::Frame::Hello {
            build: BUILD.into()
        })
    );
    let call: Frame = serde_json::from_str(
        r#"{"ui":{"call":{"id":1,"method":"rename_chat","args":{"plane":"p1","session":5,"label":"login"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        call,
        Frame::Ui(ui::Frame::Call(ui::Call {
            id: 1,
            method: "rename_chat".into(),
            args: json!({"plane": "p1", "session": 5, "label": "login"}),
        }))
    );
    let welcome = Frame::Ui(ui::Frame::Welcome {
        build: BUILD.into(),
    });
    assert_eq!(
        serde_json::to_string(&welcome).unwrap(),
        r#"{"ui":{"welcome":{"build":"0.4.0+9055f7a"}}}"#
    );
    let ok = Frame::Ui(ui::Frame::Reply(ui::Reply {
        re: 1,
        outcome: ui::Outcome::Ok(Value::Null),
    }));
    assert_eq!(
        serde_json::to_string(&ok).unwrap(),
        r#"{"ui":{"reply":{"re":1,"ok":null}}}"#
    );
    let err = Frame::Ui(ui::Frame::Reply(ui::Reply {
        re: 1,
        outcome: ui::Outcome::Err(json!("that plane is not open")),
    }));
    assert_eq!(
        serde_json::to_string(&err).unwrap(),
        r#"{"ui":{"reply":{"re":1,"err":"that plane is not open"}}}"#
    );
}

#[tokio::test]
async fn a_link_sees_that_an_edit_waits_but_never_the_change_it_would_write() {
    // #1146: an edit is allowed on the change the window drew, and only the window answers
    // one, so the change itself stays the window's: a link is told it is not shown, beside
    // which no Allow is drawn. The row stays, so the ask is seen waiting.
    let commands = Commands::default();
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let ui = ui::Server::new(BUILD, ["pending_asks"], commands.clone());
    tokio::spawn(session::serve(served.unwrap(), Sessions, Some(ui)));
    let client = Client::new(client.unwrap()).0;
    let ui = client.ui(BUILD).await.unwrap();

    let listed = ui
        .call("pending_asks", json!({"plane": 1}))
        .await
        .unwrap()
        .unwrap();

    let mut row = an_edit_waiting()["asks"][0].clone();
    row["writes"] = ui::an_edit_drawn_in_the_window_only();
    assert_eq!(listed, json!({"plane": 1, "asks": [row]}));
    assert_eq!(listed["asks"][0]["writes"]["kind"], "not-shown");
    assert!(!ui::WINDOW_ONLY.contains(&"pending_asks"));
    assert!(ui::WINDOW_ONLY.contains(&"answer_ask"));
}

#[test]
fn the_persons_devtools_are_opened_watched_and_typed_into_from_the_window_alone() {
    // #2028: DevTools reach the whole of the inspected page, so no link but the window's own
    // carries their commands.
    for command in ["inspect_browser", "watch_inspector", "inspector_input"] {
        assert!(ui::WINDOW_ONLY.contains(&command), "{command}");
    }
}
