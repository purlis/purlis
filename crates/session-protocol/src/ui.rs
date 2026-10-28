// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The UI RPC: what the app's window asks of the host beyond the session protocol (FD-26, ADR
//! 0068 §4).
//!
//! **It is private, and promises nothing across versions.** Its methods are the app's own
//! commands, generated from `ipc_commands.rs` with a typed TypeScript client, and they change
//! whenever the app does. So it is excluded from the compatibility tests that hold the session
//! protocol ([`crate::session`]), and a host serves it only to the build that it is:
//!
//! 1. The window opens it with `{"ui":{"hello":{"build":"…"}}}`, naming its build.
//! 2. The host answers `{"ui":{"welcome":{"build":"…"}}}` only when that is its own build, and
//!    the link's scope is `local-ui`, the app's window (ADR 0068 §5). Otherwise it answers
//!    `{"ui":{"refused":{"why":"…"}}}`, and the session protocol on the same link carries on:
//!    across an upgrade, the window and the host are two builds, and the session protocol alone
//!    carries that (ADR 0068 §7).
//! 3. Then `{"ui":{"call":{"id":3,"method":"rename_chat","args":{…}}}}`, answered
//!    `{"ui":{"reply":{"re":3,"ok":…}}}` or `{"ui":{"reply":{"re":3,"err":…}}}`. `method` and
//!    `args` are what the window's `invoke` sends today: the command's name, and its arguments
//!    by name. `err` is the command's own error, as its typed client reads it.
//!
//! A call before the welcome, or of a method the host does not serve, is answered `err` and
//! goes no further.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::grants::Power;
use crate::session::{Ask as LinkAsk, CallError, Peer};

/// One frame of the UI RPC, inside the control lane's `{"ui":…}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    Hello { build: String },
    Welcome { build: String },
    Refused { why: String },
    Call(Call),
    Reply(Reply),
}

/// One method call, numbered for its reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub args: Value,
}

/// The reply to the call numbered `re`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub re: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok(Value),
    Err(Value),
}

/// What answers a UI RPC method: the host's side of the app's commands.
pub trait Handler: Send + Sync + 'static {
    fn call(&self, method: String, args: Value) -> BoxFuture<'_, Result<Value, Value>>;
}

/// The UI RPC a host serves: its build, the methods it answers, and what answers them.
#[derive(Clone)]
pub struct Server {
    build: String,
    methods: BTreeSet<String>,
    handler: Arc<dyn Handler>,
}

/// The app's commands no link ever serves, whatever list a host is built with: they are the
/// window's alone, over Tauri's IPC.
///
/// `answer_ask` answers a chat's ask as the operator (HP-6). Only a human scope answers one (V16,
/// V75). On a link that is the session protocol's `answer`, which [`crate::grants`]' table
/// checks (FD-27); the UI RPC does not carry a second way to do it.
///
/// **So is every command that ends, starts or restarts a chat on the person's word** (#1488,
/// a delegated ruling, and its list as the train's reviews completed it). The rule is what a
/// command does, and it fails closed: a command that does one of these is listed here, whether
/// or not anything has yet been found to misuse it.
///
/// - **it ends a chat, a task, or every chat of a project or of the app**: `end_task` (which
///   writes a sentence in the person's name to the chat that asked), `task_ending` (the
///   question it asks first), `stop_chat`, `close_session`, `close_chat_stopping`,
///   `end_task_that_did_not_start`, `stop_all_tasks` with `all_tasks_ending` (its question,
///   #1498), `smart_close`, `stop_every_agent`,
///   `forget_chat_that_did_not_start`, `close_plane`, `forget_project`, and the two that quit
///   the app to start it again (`restart_on_the_session_bus`, `restart_to_update`);
/// - **it starts a chat or a shell as the person**: `ask_persona_chat`, `start_chat`,
///   `start_chat_here`, `open_session`, `open_shell_in_branch`, `first_task_run`,
///   `resume_session`, `retry_chat_that_did_not_start`, `reopen_finished_task`, `curate` (a
///   chat with a curation's prompt typed), and `relaunch` (the launch's answer, which starts
///   every chat the project held);
/// - **it starts a chat again**: `restart_chat_without_sandbox`, `restart_chat`,
///   `start_chat_fresh`, `ask_chat_restart`.
///
/// - **it lets a persona's committed hosts reach past this machine's sandbox** (#1362):
///   `allow_persona_hosts`, which the person at this machine's window allows as it was shown
///   there.
///
/// None is served on a link. A command added later that does one of these belongs here.
///
/// **What this rule does not cover.** The other commands that act as the person (an answer
/// to a question other than a chat's ask, typing into a chat, saying what is on screen) end,
/// start and restart nothing. They are served to a `local-ui` peer of the same build, and
/// whether they join this list awaits a ruling. Until it is made, a command of that kind added
/// since is held to the window, failing closed: `references_into_chat` (#1151), several of the
/// explorer's rows typed into a chat as one paste, and `use_skill` (#2775), a skill of the
/// chat's typed into it by Use skill….
///
/// **And every command that reads back what one chat said to another, or was sent**: what a
/// task was sent (`task_brief`, #1494), what a session and its tasks said to each other
/// (`activity`, #1495), and the question a task is paused on (`task_question`, #1496). Each
/// reads words a chat wrote, from a store kept from the project's chats, about a session the
/// caller names; no link can say which chat is asking, so none is served on one.
///
/// `answer_task_question` answers, as the person, a question a task put to its asking chat
/// (#1496). What it sends reaches a chat marked as the person's own words, so it is held to
/// what `answer_ask` is: the window's own IPC, and no link until the session protocol has a
/// scoped message for it.
///
/// **Every command that makes, widens, narrows, accepts, declines or takes back a standing
/// dispatch grant is the window's alone too** (spec #1483, train 64; [`STANDING_DISPATCH`]),
/// and so is every answer to a dispatch's question: Keep blocked keeps nothing past the chat,
/// and it is still the person's answer, which nothing on a link gives for them. **And so are
/// the reads of what stands and what waits**: they list the person's refusals, every chat's
/// grants, and what other chats asked for with what their targets work with, and a link
/// reads none of it.
/// A dispatch grant is the person's consent to what one persona's chats may ask of another,
/// given by a press in their own window; the link carries no second way to give or withdraw
/// it, whatever scope the link holds. With them, the commands of the two lists a person
/// answers from: the Notice that says a teammate's grant arrived (#1506), and the dispatches
/// refused while nobody was there (#1507), which are offered on a refused chat's word.
///
/// **And "Got it" on the Notices that the project's Internet access presets** (#1385) **or its
/// own hosts** (#1341, #1550) **changed**: each records that this person was told what chats may
/// now reach or write, so the Notice is not shown again. Only the person who read it says so; no
/// link answers it for them.
///
/// `task_branch_merge` and `dispatch_worktree_discard` are the person's acts on what a task
/// left on a branch of its own (#1511, V100-67): a merge into the branch it was cut from, and
/// a folder removed for good. Each is asked first, in the window, and no chat may cause
/// either, so neither has a form on a link.
///
/// `worktree_remove` and `worktree_merge` are the explorer's remove and merge of any branch
/// folder (#1534): the person's act on their own repo, from their window, which takes a folder
/// for good (with `force`, its uncommitted work) or moves the clone. No link carries either.
///
/// `allow_sandbox_block_for_tasks` and `keep_sandbox_block_for_tasks` answer several tasks'
/// sandbox block on one press (#1508), and `allow_sandbox_block` answers one chat's (#1538):
/// a person's answer given in their own window to the block it showed, which no link carries
/// a second way to give. `keep_sandbox_block` is one chat's Keep blocked (#1666): it refuses a
/// connection the chat's proxy holds while the person is asked, so it is theirs alone too; no
/// chat answers its own ask (V16a).
///
/// **Removal, grant, settings and credential commands are the window's alone** (#1927): what
/// takes something away for good, what gives, answers or takes back a consent, what saves a
/// setting that decides what chats may do, what installs, runs or publishes for the person,
/// and what carries a credential. Each is the person's own act, made in their own window.
/// This list is the source of which commands those are; the classes are named beside the
/// entries below.
///
/// **A link is served a command only by name** ([`SERVED_ON_A_LINK`]): every command of the
/// app is on that list or on these, and the app's test fails for one on neither or on both.
/// So a command added later is the window's alone until someone lists it as served, on
/// purpose and in a place a review reads.
///
/// `open_extension_record` starts the person's editor on this machine's extension record
/// (#1296): a program of theirs run on a machine file, from the Extensions dialog, which no link
/// asks for.
///
/// `open_in_your_editor` and `open_their_agents_md` start the person's editor on a branch's
/// file, and a terminal editor in a shell tab of its own (#1042): a shell started as the person,
/// so the window's alone.
///
/// `open_blocked_file` starts the person's editor on a file the project's save is blocked in,
/// from the Saving view: a program of theirs run on their press, which no link asks for.
///
/// `plane_tree` and `plane_file` list and open the plane root's files (issue 1916): the
/// project's own files, of no workspace's repo, which the person browses in their window. A
/// link is served a branch's files and never these, so no chat gains a second way to read the
/// project through one.
pub const WINDOW_ONLY: &[&str] = &[
    "answer_ask",
    // The clipboard's text, read for a Paste the person pressed in a terminal's menu
    // (ADR 0052 amended 2026-10-14). The clipboard is the person's, and may hold a vault's
    // value: no link reads it, so no chat, tool or hook does.
    "clipboard_text",
    // The bytes of a font a theme folder brings (#2326), from the person's and the project's
    // theme folders, which the window loads its faces from. Held to the window, failing closed:
    // a link draws with the platform's fonts.
    "theme_font",
    // The theme in force for the window that asks, which also points that window's theme watch
    // at the project it answers for (#2324). A link has no window here to watch for: it is
    // served `theme_in_force_at`, the same answer, which watches nothing (#2697).
    "theme_in_force",
    // The theme drafts of a chat, asked before the window closes it (#2332): the chat, where it
    // works, each draft and what Keep would do, for the person's Keep or Revert, which are the
    // window's alone, as closing a chat is (#2708).
    "theme_drafts_of_chat",
    // The theme specimen (#2330): a hidden window this app opens on this machine, photographed
    // and closed. Drawn for the window and for the chats this app started, on their hook
    // socket; a link has no window of its own here to draw it for.
    "theme_specimen",
    // The window's own native chrome (#2328): its appearance and colour, drawn as the theme the
    // window draws, and read back. A link has no native window of its own here.
    "draw_native_window",
    "native_window_drawn",
    // Ends.
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
    // Signing out of a forge account deletes its token (#1814, ADR 0077 §3).
    "forge_sign_out",
    // This machine's forge accounts: the hosts and logins the person signed in as, which no
    // link reads (#1816).
    "forge_accounts",
    // An existing gh or glab login (#1376, ADR 0077 §5): which ones this machine has, and the
    // person's yes that copies one into purlis's keyring item. No link offers or imports one.
    "forge_cli_logins",
    "forge_import_cli_login",
    // Signing in to a forge through the browser by the device flow (#729, ADR 0077 §3): the
    // start, the wait that signs the account in, and the cancel. No link signs anyone in.
    "forge_device_sign_in_start",
    "forge_device_sign_in_wait",
    "forge_device_sign_in_cancel",
    // The browser sign-in to gitlab.com (#730, ADR 0077 §3): it opens the person's browser and
    // a loopback listener on this machine, and ends with a token in the keychain. No link
    // starts one or stops one.
    "forge_pkce_sign_in",
    "forge_pkce_sign_in_cancel",
    // This machine's profiles (#2034, ADR 0095): the list, and the person's make, import and
    // remove of one, each asked in the window. What a chat runs as is never a chat's to change.
    "machine_profiles",
    "machine_profile_create",
    "machine_profile_import",
    "machine_profile_removal",
    "machine_profile_remove",
    // Typing a profile's sign-in into the person's own shell tab (#2035): the harness's own
    // sign-in, run as the person, never a chat's to start.
    "type_profile_sign_in",
    "dismiss_sign_in",
    "machine_profile_pin",
    // A sandbox-only Codex profile made from its Ask (#2037): the person's own profile.
    "make_sandbox_only_profile",
    "dismiss_sandbox_only",
    // Whether a folder on this machine is in a project (#1822): the welcome's question before
    // it offers to make one. A probe of the person's folders, which no link needs.
    "folder_is_a_project",
    "restart_on_the_session_bus",
    "restart_to_update",
    "restart_to_update_choosing",
    // Starts.
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
    // Starts again.
    "restart_chat_without_sandbox",
    "restart_chat",
    // A chat moved to or from a pool's runner (#2048): a new run where it moves.
    "move_chat",
    "move_chat_now",
    "start_chat_fresh",
    "ask_chat_restart",
    // Reads what a chat was sent or said to another.
    "task_brief",
    "activity",
    "task_question",
    // Answers as the person.
    "answer_task_question",
    // What stands and what waits: the person's and every chat's, so never a link's to read.
    "dispatch_grants_needed",
    "asks_waiting",
    "dispatch_standing",
    "dispatch_grants",
    // The person's answers on a dispatch's Notice: every one of them, the no included.
    "allow_dispatch",
    "allow_dispatch_anywhere",
    "keep_dispatch_blocked",
    "never_dispatch",
    // Keep blocked on a sandbox host ask (#1690): the person's no, told to the app.
    "forget_sandbox_block",
    // Keep on a chat's question whether to remove a workspace (qw356): the person's no. No
    // chat answers its own ask. The removal has no command here: it is the window's own Delete
    // workspace, which the question opens.
    "keep_asked_workspace",
    // What a chat's browser may reach, on its first browser call's Ask (#2013): the person's
    // answer alone. No chat answers its own Ask.
    "allow_chat_browser",
    "keep_chat_browser_off",
    // Whether a persona's browser logins may be filled on a pool's runner (#2050): the person's
    // answer alone. No chat opts itself in.
    "answer_pool_login",
    // Whether this desktop sends a persona's tool its vault values on a pool's runner (#2047):
    // the person's answer alone. No chat allows its own tool.
    "answer_pool_tool",
    // A chat's browser sites (#2018): the person's answer alone. No chat allows its own
    // browser a site.
    "allow_browser_sites",
    "keep_browser_sites_blocked",
    // The sites a browser the person opened asked about (#2176): their answer alone.
    "allow_person_browser_sites",
    // Apply and Dismiss on a chat's proposed change (#1975): only the person applies a change to
    // purlis's own configuration, and no chat answers its own proposal (PC-3).
    "apply_proposed_change",
    "dismiss_proposed_change",
    // And the change whole, for its own tab (PC-7): what the configuration file holds is the
    // person's to read, never a chat's through the UI's link.
    "proposed_change",
    // Remove and Dismiss on the Notice of the old `[harness_plugins]` keys (#3467): Remove holds
    // purlis's own proposed change for the person to apply, and the answer is the person's.
    "remove_old_harness_plugins",
    "dismiss_old_harness_plugins",
    // Dismiss on a chat's Notice that a persona's MCP server could not start (qw381): the
    // person's answer, which no chat gives for them.
    "put_away_mcp_failure",
    // Dismiss on a chat's Notice that its program ended with its session host (#2007): the
    // person's answer.
    "put_away_lost_chat",
    // Try again on the Notice that the session host is failing (#2007): it starts the host
    // every chat runs in, on the person's word.
    "start_the_session_host_again",
    // Dismiss on a task's Notice that it has gone quiet (#1981): the person's answer.
    "put_away_task_quiet",
    // The Inbox's updates (#1693): what happened to the person's chats, and what they put away.
    "inbox_updates",
    "note_inbox_updates",
    "settle_inbox_updates",
    // Settings' table: every change of what stands.
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
    // A teammate's grant arriving: its list and its answers.
    "dispatch_arrival",
    "answer_dispatch_arrival",
    "dispatch_gone_told",
    // A dispatch refused while nobody was there: its list and its answers.
    "dispatch_away",
    "allow_dispatch_away",
    "dismiss_dispatch_away",
    "never_dispatch_away",
    // The person's "Got it" on a Notice that the project's presets or its hosts changed.
    "acknowledge_project_presets",
    "acknowledge_project_hosts",
    // ...and the person's "Got it" on the Notice of the project's browser sites (#2443), which
    // lets them reach this machine's browsers.
    "acknowledge_project_browser_sites",
    // What a task left on a branch of its own: the person's merge or discard (#1511).
    "task_branch_merge",
    "dispatch_worktree_discard",
    // ...and the delete of a merged branch whose folder is gone (#1472).
    "task_branch_delete",
    // The explorer's remove and merge of any branch folder (#1534).
    "worktree_remove",
    "worktree_merge",
    // The answer to a chat's sandbox block (#1538), and one to several tasks' (#1508).
    "allow_sandbox_block",
    "allow_sandbox_block_for_tasks",
    "keep_sandbox_block_for_tasks",
    // Keep blocked on one chat's: refuses what its proxy holds and tells the chat (#1666).
    "keep_sandbox_block",
    // Point kubectl at it (qw384b): the person's kubeconfig contexts, read for their press,
    // and the copy of one made for a chat. What it reads is the person's, and what it makes
    // widens what that chat reaches, so no link lists or makes either.
    "kube_contexts",
    "point_kubectl",
    // The person names what a block did not (#1884, ADR 0093): the sockets and services this
    // machine offers to choose from, the choice itself, and taking up a block a chat only read
    // back. The choice is the person's, so no link lists, makes or prompts it.
    "sandbox_block_choices",
    "choose_sandbox_block_target",
    "take_up_sandbox_block",
    // Widens what a persona's chats reach on this machine (#1362).
    "allow_persona_hosts",
    // The person's answer to what a project declares it expects (ADR 0093): the Allow widens
    // what every chat of the project reaches here, and "Not now" is theirs to say, so no chat
    // answers the question before it is asked.
    "allow_project_expects",
    "decline_project_expects",
    // The person's answer to the project's sandbox off (#2071, SO-8): theirs alone, so no chat
    // turns its own sandbox off through a link.
    "answer_team_sandbox_off",
    // Remove on the Allowed list (#1927): the person's standing answers are taken back in
    // their own window.
    "revoke_sandbox_grant",
    // Removal commands (#1927): what takes something away for good.
    "workspace_remove",
    "drop_repo",
    "drop_repo_membership",
    "persona_remove",
    "vault_remove",
    "vault_secret_delete",
    "todo_forget",
    "todo_forget_done",
    "clear_finished_tasks",
    // Grant commands (#1927): a consent given, answered or taken back.
    "allow_refused_vault",
    "keep_vault_blocked",
    "put_away_refused_vault",
    "allow_persona_grants",
    "confirm_sandbox_host",
    "list_grantable_folder",
    "unlist_grantable_folder",
    "approve_extension",
    "forget_extension",
    "approve_plane",
    // The trust ask about a project already open (#2647): only the person's press reviews it.
    "review_plane",
    "approve_profile",
    "revoke_approval",
    "answer_sandbox_offer",
    "rearm_agents",
    // Settings commands (#1927): a saved setting decides what chats may reach and run.
    "save_project_settings",
    "save_workspace_settings",
    "move_project_settings",
    "add_sandbox_host",
    "remove_sandbox_host",
    "add_project_forge",
    "remove_project_forge",
    "add_project_profile",
    "remove_project_profile",
    "rename_project_profile",
    "persona_set_profile",
    // Install, run and publish commands (#1927): a program installed or run, or work sent
    // out, as the person.
    "install_extension",
    "set_extension_on",
    "pick_extension",
    "install_update",
    "set_update_channel",
    "run_action",
    "extension_programs_run",
    "install_cli_on_path",
    "type_installer",
    "type_sandbox_install",
    "plane_doctor_fix",
    "plane_doctor_fix_identity",
    "change_land",
    "change_push",
    "publish_project",
    "create_project_on_forge",
    "save_plane",
    "save_repo",
    "choose_plane_mode",
    // Typed into a session as the person: one of the explorer's rows, as several are
    // (#1151), and the keys themselves.
    "reference_into_chat",
    "send_input",
    "send_input_bytes",
    // Starts the person's editor on this machine's extension record (#1296).
    "open_extension_record",
    // Allow on Settings' Blocked lately (#1662): widens what every chat here reaches.
    "allow_blocked_host",
    // A service on this Mac added by its port on Settings' Network page (#1970): widens what
    // every chat here, or one chat, reaches.
    "allow_sandbox_port",
    // Several of the explorer's rows typed into a chat as the person (#1151): held to the
    // window until typing into a chat has its ruling.
    "references_into_chat",
    // A skill of a chat's typed into it as the person by Use skill… (#2775): held to the window
    // until typing into a chat has its ruling, as a reference is.
    "use_skill",
    // Why this purlis opens a project read-only (#826): the window's tab sentence, which no
    // link needs.
    "project_read_only",
    // The first task's runs read back from the clone, each with its diff command (#945): the
    // window's First task tab alone.
    "first_task_started",
    // What the person kept blocked here (#1700): their answers, read for the Inbox alone.
    "sandbox_kept_blocked",
    // The project's harness declarations in Settings, and Open file on one, which starts the
    // person's editor (#1389).
    "project_harness_declarations",
    "open_harness_declaration",
    // Opens Apple's installer for the developer tools that bring git (#1813): the person's
    // press on the window's guided screen, which starts a system install on this machine.
    "install_developer_tools",
    // Get a project by URL (#1815): the folder it would go into, and the person's Cancel of
    // the clone (the clone itself takes a channel). A clone writes a folder on this machine
    // with the person's own git sign-in, so it is started and stopped from the window alone.
    "project_url_folder",
    "cancel_get_project",
    // Where the person is in the guided steps (#1817), read and kept for the next launch:
    // setting up this machine is the window's, never a link's.
    "onboarding_progress",
    "keep_onboarding_progress",
    // Create a project on GitHub or GitLab (#1819): whom the person's account can make a repo
    // for, the folder a name goes into, and the person's Cancel of the creation (the creation
    // itself takes a channel). It makes a repo on their forge account and a folder on this
    // machine, so it is started and stopped from the window alone.
    "forge_owners",
    "new_project_folder",
    "cancel_create_project",
    // The repos the person's forge accounts reach, listed as them (#1818): read from the window
    // alone, as the accounts are.
    "forge_repo_list",
    // Whether a project lives only on this machine (#1820): the window's line on the project's
    // tab and in its settings, beside its Publish (which takes a channel), which no link needs.
    "project_lives_here",
    // The person's answers to the Saving view's question about a clone's remote (#1848,
    // #1852): each settles the remote automatic saves push to with their sign-in, so no link may
    // press it.
    "confirm_remote",
    "keep_recorded_remote",
    // Update from remote (qw348): the app brings the project's incoming commits in, which
    // moves its working tree under every chat in it. The person's own press, and no link's:
    // a chat that asks for it raises an item the person reads, and presses nothing.
    "update_project",
    // Merge a save's pull request as it stands, failed and missing checks included (the
    // operator's ruling P1, 2026-10-13): the person's own, explicit press and nothing else's.
    // A save and the app's own look never merge past a red check, and no link or chat may.
    "merge_save_request",
    // The fresh look the merge's question is drawn from: it asks the forge with the person's
    // sign-in, and merges a request whose checks all passed, as every look does.
    "look_at_save_request",
    // Fix on a save's red pull request (the operator's ruling P2, 2026-10-13): it starts a
    // chat at once on a message purlis wrote, which nothing else in purlis does for a press.
    // The person's own press in the window, and no link's or chat's.
    "fix_save_request",
    // Resolving a save's conflict (qw382): the window runs the rebase, stages exactly the files
    // in conflict and continues or aborts it, which moves the project's tree; and a chat is
    // started on a message purlis wrote, to edit those files and run no git. The person's own
    // presses in the window, and no link's or chat's.
    "settle_save_conflict",
    "resolve_save_in_a_chat",
    // Filing a sandbox block's report (#806): it opens an issue on purlis's tracker as the
    // person, with their purlis sign-in, so only their own window's press files it.
    "file_sandbox_block_report",
    // Open in your editor on a branch's file, and Open file on a start notice's AGENTS.md: a
    // terminal editor opens in a shell tab of its own (#1042), so each starts a shell as the
    // person.
    "open_in_your_editor",
    "open_their_agents_md",
    // Open the file on a blocked save's finding, in the Saving view: starts the person's
    // editor on the file the save is blocked in.
    "open_blocked_file",
    // The plane root's files (issue 1916): the project's own, listed and opened in the
    // person's window alone.
    "plane_tree",
    "plane_file",
    // The Fix on a stale repo's Notice (#1985): it switches a workspace's repo to another
    // branch, or points its `origin` at another address. The person's own press in the window.
    "fix_stale_clone",
    // The plain-file vaults offered to move into the keychain before a person turns their
    // sandbox down (#2071): read in their window alone.
    "plain_file_vaults",
    // Your levels waiting for Confirm, and the Confirm that makes one count at a start
    // (#2071, ADR 0096 §7): the window's alone, as the record they read and write is.
    "sandbox_levels_waiting",
    "confirm_sandbox_level",
    // A browser started, watched and ended (#2012): a program started as the person, in their
    // own window. Chats get browser tools of their own later (#2013).
    "open_browser",
    "watch_browser",
    "unwatch_browser",
    "close_browser",
    // Pinning a chat's browser past its chat, and handing a browser no chat uses to one (#2022):
    // the person's acts alone. No chat pins its own browser or takes one.
    "pin_chat_browser",
    "hand_browser_to_chat",
    // Where the project's browsers were at the last quit, and putting that away (#2022): the
    // person's, in their window.
    "browsers_at_quit",
    "put_away_browsers_at_quit",
    // This machine's browser limits (#2023): the person's to read and set, in their window.
    "browser_limits",
    "set_browser_limit",
    // This machine's Skill studio preview (#2765): the person's to turn on, in their window.
    "set_skill_studio_on",
    // Share in the Skill studio (#2767): moves a local skill into the shared folder, so the next
    // save commits it to the team. The person's own press in their window.
    "skill_share",
    // The Skill studio's changes (#2770): making, saving, renaming, moving, deleting and
    // restoring a skill, its grant and a workspace's turn-off. Each is one commit to the
    // project, the person's own press in their window. A chat's change is a draft (SK-9).
    "skill_new",
    "skill_save",
    "skill_rename",
    "skill_move",
    "skill_delete",
    // A skill delete a crash or a quit cut short, finished in one commit or put back as the
    // studio opens (SK-35, #3013): the person's window alone, as Delete is.
    "skill_deletes_cut_short",
    "skill_restore",
    "skill_grant",
    "skill_turn",
    // A skill pinned or unpinned for a workspace's or a persona's chats (SK-15d, #3232): one
    // commit to the project, the person's own press in their window, as a turn-off is.
    "skill_pin",
    // A skill's tab (#2782): reverting it to a version, deprecating, archiving or restoring it,
    // and naming its owners. Each is one commit to the project, the person's own press in their
    // window.
    "skill_revert",
    "skill_lifecycle",
    "skill_owners",
    // A chat's draft skills read, accepted or rejected, and a chat opened to draft one or to try
    // one (SK-9, #2772): a draft is what a chat wrote, read by the person in their window as a
    // proposed change is; an accept is one commit to the project, and a chat starts a program as
    // the person. The person's own press in their window, as every skill change is.
    "skill_drafts",
    "skill_draft_accept",
    "skill_draft_reject",
    "skill_draft_chat",
    "skill_draft_try",
    // A grade of a skill started or stopped (SK-16, #2779): it runs chats on the person's
    // profile, each started on a trigger test's query, and records the grade in the project.
    // The person's own press in their window, after the plan showed what it costs.
    "skill_grade_start",
    "skill_grade_stop",
    // A grade of a router started (SK-15e, #3233): as a skill's grade, it runs chats on the
    // person's profile and records the grade in the project. Stopped by `skill_grade_stop`.
    "router_grade_start",
    // A grade's token cap (#2779): what this machine's grades may spend, in its own settings
    // file. The person's own press in their window.
    "skill_grade_set_cap",
    // Grading after each edit turned on or off (SK-58, #3238): on, it spends the person's
    // account at each edit they save, within what the plan showed. The person's own press in
    // their window, never a link's.
    "skill_grade_after_edit_on",
    "skill_grade_after_edit_off",
    // Accepting a workspace or persona folder a chat could have made (SK-9): its skills then
    // reach chats, so only the person's own press in their window does it.
    "skill_level_accept",
    // Skill sources (#2773): each reaches a git repository the person named, a private one with
    // a credential read from a vault, and an install or an update is one commit to the project.
    // The person's own press in their window, never a link's.
    "skill_sources",
    "skill_sources_offered",
    "skill_source_add",
    "skill_source_remove",
    "skill_source_confirm",
    "skill_source_browse",
    "skill_install_review",
    "skill_install",
    "skill_updates",
    "skill_update_review",
    "skill_update",
    // A harness's plugin or skill turned on or off in the Skill studio (#2769): it writes this
    // machine's project file, or the workspace's settings. The person's own press in their
    // window.
    "skill_studio_switch",
    // Why browsers cannot start for the engine's sandbox, and choosing this machine's Chrome
    // for every browser (#2835): which program runs as the person's browsers' engine is theirs
    // to choose, in their window.
    "engine_sandbox",
    "use_this_machines_chrome",
    // A persona's Skills section (#2768): adds a skill to the persona's definition or takes one
    // out, as its profile is set. The person's own press in their window.
    "persona_skill",
    // Putting a chat's missing-skills Notice away (#2768), on the chat's pane in the person's
    // window, as an MCP failure's is.
    "put_away_chat_skills_missing",
    // The browser engine's first download, and cancelling it (#2207): the person's, in their
    // window, of a start their window made.
    "engine_downloads",
    "cancel_engine_download",
    // This machine's Docker pools (#2679): adding one runs a container on the person's machine
    // and pairs it with this desktop's key, and removing one removes it, so only their own press
    // in their window does either.
    "docker_pools",
    "add_docker_pool",
    "remove_docker_pool",
    "docker_pool_link",
    "docker_pool_version",
    // This machine's profiles on a pool's runner (#2049): keeping one there sends its name to a
    // runner, so only the person's own look in their window does.
    "pool_profiles",
    // The Devices view (#2052): it reads this machine's runners and links to each, so only the
    // person's own window asks it.
    "devices",
    "device_runner",
    // The projects a runner would not hold again (#2846): what this window was refused.
    "projects_not_held",
    // Where a workspace's chats run on this machine (#2046): a pool's runner gets the
    // workspace's code, so only the person's own choice in their window sets it.
    "chats_run_on",
    "set_chats_run_on",
    // What a workspace's sends to a runner leave out (#2046): the person's list, in their window.
    "send_leaves_out",
    "set_send_leaves_out",
    // A runner chat's held change to the project (#2046): only the person's press applies it,
    // and the change whole, for its own tab, is the person's to read.
    "apply_runner_write",
    "dismiss_runner_write",
    "runner_write",
    // Taking a browser over, driving it and handing it back (#2014): what the person types into
    // a page is theirs, and only the window, where they see it, sends it.
    "take_over_browser",
    "hand_back_browser",
    "browser_input",
    "size_browser",
    // The person sends a browser of their own to an address they typed (#2176): theirs, from
    // its view tab, as their window shows it.
    "go_browser",
    // A chat's browsers, listed to be watched (#2024): the window's alone, as watching is.
    "chat_browsers",
    // A chat's browser timeline and its screenshots (#2017): the window's alone, as watching
    // is, since they hold pictures of the chat's pages.
    "chat_browser_timeline",
    "chat_browser_timeline_shot",
    // Browser proof on a pull request (#2026): the person picks the screenshots, sees what goes,
    // and attaches it with their own sign-in, so only their presses in the window make any.
    "browser_proof_requests",
    "show_browser_proof",
    "attach_browser_proof",
    // The person's own browsers a crash of the window left running, for their view tabs
    // (#2025): the window's alone, as watching is.
    "browsers_taken_back",
    "browser_viewed",
    // The person's own theme choice, and the move of their old theme file into a theme folder
    // (#2323): each writes how their window looks on this machine, and the move moves their
    // file, so only their press in the window makes either.
    "theme_choose",
    "theme_move_into_a_folder",
    // Settings › Theme's gallery (#2331): the person's machine default, a theme folder of their
    // own made by Duplicate as my theme, and Open folder, which starts their file manager on a
    // theme's folder. Each is the person's press in the window.
    "theme_choose_machine",
    "theme_duplicate",
    "theme_open_folder",
    // The value of a theme option at a level (#2527): it writes how the person's window looks
    // on this machine, so only their press in Settings › Theme makes it.
    "theme_choose_option",
    // Settings › Theme's New theme and Edit with a chat (#2333): a chat started with a prompt
    // typed into it, as `curate` starts one, by the person's press in the window.
    "theme_chat",
    // A chat's theme draft (#2332): Keep writes it into a theme folder, Revert and the revert key
    // delete drafts, and the moves up a level write the project's or the person's themes. Each
    // is the person's command, never a chat's.
    "theme_draft_keep",
    "theme_draft_revert",
    "theme_drafts_revert_here",
    "theme_move_to_project",
    "theme_copy_to_machine",
];

/// **The commands of who may dispatch to whom**, by name (spec #1483): every one is on
/// [`WINDOW_ONLY`] (a test holds that), and the list stands on its own, so a window-only
/// command of another kind added to [`WINDOW_ONLY`] is never counted as one of these.
pub const STANDING_DISPATCH: &[&str] = &[
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

/// The commands that take a vault's sign-in from the person and use it (#1527, ADR 0052 as
/// amended 2026-10-09): a credential the person gives purlis reaches it through the window's
/// own IPC, from the window that began the set-up, and is never carried by a link. The window's
/// alone, as [`WINDOW_ONLY`]'s are.
///
/// **With them, every credential command** (#1927): the whole value-bearing class (ADR 0052's
/// `vault_values`), which the app's test holds this list to, and the commands that give a
/// vault a secret's value or its identity.
pub const WINDOW_ONLY_CREDENTIALS: &[&str] = &[
    "vault_secret_reveal",
    "vault_secret_copy",
    // A secret's value given to a vault, or a vault's identity put or moved.
    "vault_secret_add",
    "vault_secret_set",
    "vault_secret_rename",
    "vault_identity_put",
    "vault_identity_move",
    "finish_moving_vaults",
    // A plain-file vault's values moved into the keychain (#2071).
    "move_vault_to_keychain",
    "vault_setup_accounts",
    "vault_setup_begin",
    "vault_setup_test",
    "vault_setup_create",
    "vault_setup_change",
    "vault_setup_cancel",
    // A forge token pasted for a sign-in (#1814, ADR 0077 §4): the same rule.
    "forge_sign_in_with_token",
];

/// **A part of a reply that is the window's alone** (#1146): a link is served the method, and
/// its reply with `field`, wherever in the reply it is and not null, replaced by
/// [`Self::in_its_place`].
#[derive(Debug, Clone, Copy)]
pub struct WindowOnlyField {
    pub method: &'static str,
    pub field: &'static str,
    /// What a link is served where the field was.
    pub in_its_place: fn() -> Value,
}

/// **The parts of replies that are the window's alone** ([`WindowOnlyField`]).
///
/// `pending_asks` lists what waits on the person, and a link still sees each ask waiting; but
/// an edit's change (`writes`) is what the window draws before it offers that edit's Allow, and
/// only the window gives that Allow. So a link is served, in its place, the window's own form
/// for a change not shown (`app/src-tauri/src/askwrites.rs`, whose test holds the two alike),
/// beside which no Allow is offered.
pub const WINDOW_ONLY_FIELDS: &[WindowOnlyField] = &[WindowOnlyField {
    method: "pending_asks",
    field: "writes",
    in_its_place: an_edit_drawn_in_the_window_only,
}];

/// **Whether a call names the plane root** (issue 1916): `files_watch` with a folder of no
/// workspace and no repo, which is how the window names a folder of the project's own. A link
/// is served a branch's folders and never the plane root's, so such a call is refused before
/// its handler is asked ([`THE_PLANE_ROOT_IS_THE_WINDOWS`]). A folder that leaves either name
/// out is counted as naming none, and so is a call that does not say its folders as a list:
/// only a call whose every folder names a branch goes on to the handler.
pub fn names_the_plane_root(method: &str, args: &Value) -> bool {
    let named = |name: &Value| name.as_str().is_some_and(|name| !name.is_empty());
    method == "files_watch"
        && !args["folders"].as_array().is_some_and(|folders| {
            folders
                .iter()
                .all(|one| named(&one["workspace"]) || named(&one["repo"]))
        })
}

/// What a link is told when its call names the plane root.
pub const THE_PLANE_ROOT_IS_THE_WINDOWS: &str =
    "The project's own files are listed and watched in purlis's window only.";

/// What a link is served for an edit's change: not shown, and why.
pub fn an_edit_drawn_in_the_window_only() -> Value {
    serde_json::json!({
        "kind": "not-shown",
        "why": "This change is shown in purlis's window only: allow it there.",
    })
}

/// `value`, the reply to `method`, as a link is served it: each [`WINDOW_ONLY_FIELDS`] field of
/// that method replaced, at any depth.
fn as_a_link_is_served(method: &str, mut value: Value) -> Value {
    for kept in WINDOW_ONLY_FIELDS
        .iter()
        .filter(|kept| kept.method == method)
    {
        replace(&mut value, kept);
    }
    value
}

fn replace(value: &mut Value, kept: &WindowOnlyField) {
    match value {
        Value::Object(object) => {
            for (key, inner) in object.iter_mut() {
                // An absent part (null) is left as it is: there is nothing of the window's in it.
                if key == kept.field && !inner.is_null() {
                    *inner = (kept.in_its_place)();
                } else {
                    replace(inner, kept);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|inner| replace(inner, kept)),
        _ => {}
    }
}

/// **The commands a link may be served**, each by name (#1927). Every command of the app
/// (`ipc_commands.rs`) is here or is the window's alone ([`window_only_commands`]), never both
/// and never neither: the app's test holds that, so a command added later is refused to a
/// link until someone adds it here on purpose. What is here reads, or acts on what the window
/// shows (a tab, a pin, a search, a terminal's bytes), or is one of the person's edits whose
/// place among the window's alone awaits the ruling [`WINDOW_ONLY`] names.
pub const SERVED_ON_A_LINK: &[&str] = &[
    "about_charter",
    "activity_chat",
    "adopt_layout",
    "agents_stopped",
    // The runners a re-arm re-arms with this machine (#2051): a read, as whether it is stopped.
    "runners_a_rearm_reaches",
    "alerts_everywhere",
    "arrange_workspace_pins",
    "ask_persona_offer",
    "ask_to_quit",
    "branch_ahead_behind",
    "branch_status",
    "branch_tree",
    "branch_watch",
    "cancel_smart_close",
    "change_land_question",
    "change_push_question",
    "charter_windows",
    "chat_attended",
    "chat_doings",
    "chat_in_front",
    // A chat whose program ended with its session host (#2007): what its pane says.
    "chat_lost_with_the_host",
    "chat_moves",
    "chat_network",
    "chat_order",
    "chat_skill_runs",
    // The skills Use skill… offers a chat, and its skill budget (#2775): a read.
    "chat_skills",
    "chat_states",
    "chat_usage",
    "chat_work_item",
    "chat_work_link",
    "chat_work_unlink",
    "chats_on_older_sandbox",
    "chats_plane_updated",
    // Which chats are reconnecting to their runner (#2496): what their panes start from.
    "chats_reconnecting",
    "chats_that_would_not_start",
    "check_for_update",
    "check_sandbox_host",
    "clone_repo",
    "close_chat_tab",
    "copy_branch_path",
    "create_project",
    "curation_offers",
    "dispatch_limits",
    "dispatch_worktree_loss",
    "dispatches",
    "extension_commands",
    "extension_facts",
    "extension_icon_themes",
    "extension_panels",
    "extension_views",
    "extensions_on",
    "files_watch",
    "find_files",
    "find_files_end",
    "finished_tasks",
    "first_frame",
    "first_run_found",
    "forge_token_help",
    "get_project",
    "git_presence",
    "grantable_folders",
    // A link's close of chats with theme drafts, handed to the window that holds the project
    // (#2761): the window asks the person Keep or Revert, and closes nothing unasked. Answered
    // only whether it was handed.
    "hand_drafts_close_to_the_window",
    "harness_cards",
    "harness_setup_found",
    "hide_window",
    "ignore_needs_you",
    "import_instructions",
    "inbox_shown",
    "installed_extensions",
    "locate_project",
    "mcp_failures",
    // What a chat's missing-skills Notice says (#2768), read as an MCP failure is.
    "chat_skills_missing",
    "memory_archive",
    "memory_archived",
    "memory_create",
    "memory_edit",
    "memory_move",
    "memory_read",
    "memory_scopes",
    "memory_unarchive",
    "move_projects",
    "move_their_agents_md_aside",
    "open_chat_tab",
    "open_local_project",
    "open_plane",
    "open_planes",
    "open_repo",
    "open_view",
    "opened_chats",
    "owed_restarts",
    "pending_asks",
    "put_away_chat_move",
    "persona_chats_of",
    "persona_create",
    "persona_edit",
    "persona_grants_held",
    "persona_mark_set",
    "persona_marks",
    "persona_profile",
    "pick_project",
    "piece_file",
    "pin_chat",
    "pin_on_this_machine",
    "pin_project",
    "pin_workspace",
    "plane_at_launch",
    "plane_doctor",
    "plane_doctor_identity",
    "plane_fetch",
    "plane_pin",
    "plane_pins",
    "plane_remote_readers",
    "plane_root_panels",
    "plane_saving",
    "plane_sidebar",
    "planes_to_restore",
    "project_extensions",
    "project_icons_drawn",
    "project_saving_in_force",
    "project_settings",
    "project_theme",
    "project_theme_drawn",
    "projects_handed",
    "quit",
    "quit_cancelled",
    "reachable_repos",
    "recent_planes",
    "relaunch_ask",
    "rename_chat",
    "reopened_focus",
    "reopened_views",
    "repo_instructions",
    "resize_session",
    "reveal_branch_path",
    // Where the chats the away summary counts run, and their runners' branches (#2051): a read.
    "runners_away",
    "running_sessions",
    "sandbox_block_report",
    "sandbox_grants",
    "sandbox_network",
    "sandbox_state",
    "search_files",
    "search_files_end",
    "search_files_more",
    "see_on_this_machine",
    "seen_on_this_machine",
    "session_bus",
    // Whether the session host is failing, and what the window says of it (#2007).
    "session_host_failing",
    "session_record",
    "set_dismissed",
    "show_window_holding",
    "skill_details",
    "skill_diff",
    // What a router's grade runs (SK-15e, #3233): read only.
    "router_grade_plan",
    // A grade's kept chat and its scorecard (SK-59, #3239), and a skill's grading after each
    // edit (SK-58, #3238): read only.
    "skill_grade_after_edit",
    "skill_grade_chat",
    "skill_grade_plan",
    "skill_grade_scorecard",
    "skill_lint",
    "skill_move_needs_grant",
    "skill_panels",
    // Every persona's own skills, for the Skills view (#3266): read only.
    "skill_persona_skills",
    "skill_revert_widens",
    "skill_runs",
    "skill_stats",
    "skill_studio",
    "skill_studio_on",
    "skill_templates",
    "skill_text",
    "skill_users",
    "skills_deleted",
    "smart_close_offer",
    "smart_closing",
    "stale_clones",
    "start_options",
    "stopping_chats",
    "tab_shows",
    "take_repos",
    "task_branch_merge_question",
    "task_changes",
    "task_failure_seen",
    "task_folder_shared",
    "task_quiet",
    "tasks_sharing_a_folder",
    "tasks_used",
    "template_that_fits",
    // What Settings › Theme's gallery reads (#2331): its cards, and what a card draws while the
    // pointer is on it. Nothing is written.
    "theme_card_drawn",
    "theme_gallery",
    // The theme in force at a project and workspace, with no window and nothing watched
    // (#2697): what a link is served in place of `theme_in_force`.
    "theme_in_force_at",
    // The person's density and font override, read by the engine (#2326): no file, no state.
    "theme_person",
    "theme_levels_adopt",
    "this_machine",
    "title_bar_room",
    "todo_add",
    "todo_done",
    "todo_read",
    "todo_reopen",
    "unwatch_session",
    "update_channel",
    "use_built_in_theme",
    "use_default_layout",
    "vault_create",
    "vault_list",
    "vault_open",
    "vault_refresh",
    "vault_refusals",
    "vaults_to_move",
    "watch_session",
    "what_a_restart_ends",
    "what_changed",
    "window_focus",
    "window_holds_planes",
    "window_showing",
    "window_views",
    "workspace_at_risk",
    "workspace_create",
    "workspace_focused",
    "workspace_live",
    "workspace_live_preview",
    "workspace_panels",
    "workspace_rename",
    "workspace_repos",
    "workspace_saving",
    "workspace_settings",
    "workspace_starts_fresh",
    // The person's own edit of a workspace's vision (#3344), through the core's brokered
    // write: served as `workspace_create`, which takes a vision too, is.
    "workspace_vision",
    "worktree_add",
    "worktree_done",
    "worktree_list",
    "worktree_of_chat",
    "write_layout",
];

/// Whether a link may be served `method`: one of [`SERVED_ON_A_LINK`], and not the window's
/// alone.
pub fn served_on_a_link(method: &str) -> bool {
    SERVED_ON_A_LINK.contains(&method) && !window_only(method)
}

/// **Every command that is the window's alone**, each once: [`WINDOW_ONLY`]'s and
/// [`WINDOW_ONLY_CREDENTIALS`]' (#1542). The one reading of the two lists: a command is
/// declared the window's alone by adding it to one of them, and whatever asks which commands
/// those are (a host's served methods, the client a link is generated, a test that holds the
/// rule) asks here or [`window_only`], so none of them keeps a list or a count of its own.
pub fn window_only_commands() -> impl Iterator<Item = &'static str> {
    WINDOW_ONLY.iter().chain(WINDOW_ONLY_CREDENTIALS).copied()
}

/// Whether `method` is the window's alone: one of [`window_only_commands`].
pub fn window_only(method: &str) -> bool {
    window_only_commands().any(|command| command == method)
}

impl Server {
    /// `methods` is the app's command list (`ipc_commands.rs`), or the part of it this host
    /// answers. **Only a method the served list names is served** ([`served_on_a_link`]): one
    /// that is the window's alone, and one on neither list, is refused before `handler` is
    /// asked, whatever the host passes. One filter, so a command reaches a link by being
    /// listed as served and in no other way.
    pub fn new(
        build: impl Into<String>,
        methods: impl IntoIterator<Item = impl Into<String>>,
        handler: impl Handler,
    ) -> Server {
        Server {
            build: build.into(),
            methods: methods
                .into_iter()
                .map(Into::into)
                .filter(|method: &String| served_on_a_link(method))
                .collect(),
            handler: Arc::new(handler),
        }
    }

    /// [`Self::new`] answering `methods` whatever they are called: for this crate's own tests
    /// of the link itself, which call methods no app has. Only a test build has it
    /// (`any-stream` is turned on from `[dev-dependencies]` alone), so no host serves a
    /// method the served list does not name.
    #[cfg(any(test, feature = "any-stream"))]
    pub fn of_any_methods_for_tests(
        build: impl Into<String>,
        methods: impl IntoIterator<Item = impl Into<String>>,
        handler: impl Handler,
    ) -> Server {
        Server {
            build: build.into(),
            methods: methods.into_iter().map(Into::into).collect(),
            handler: Arc::new(handler),
        }
    }
}

/// How [`Serving::answer`] answers a frame: at once (or not at all), or once its handler has.
pub(crate) enum Answered {
    Now(Option<Frame>),
    Later(BoxFuture<'static, Frame>),
}

/// The host's UI RPC on one link: whether the window has been welcomed.
pub(crate) struct Serving {
    server: Option<Server>,
    open: bool,
}

impl Serving {
    pub(crate) fn new(server: Option<Server>) -> Serving {
        Serving {
            server,
            open: false,
        }
    }

    /// The host's answer to one frame. A hello, and a call refused before its handler is
    /// asked, are answered at once, so a call is judged by what the link had said hello to
    /// when it came. A call the handler answers is answered later, off the link's loop, so one
    /// that waits holds back nothing else on the link (#1095).
    pub(crate) fn answer(&mut self, frame: Frame, peer: &Peer) -> Answered {
        match frame {
            Frame::Hello { build } => Answered::Now(Some(self.hello(&build, peer))),
            Frame::Call(Call { id, method, args }) => {
                let reply = move |outcome| Frame::Reply(Reply { re: id, outcome });
                match (&self.server, self.open) {
                    (Some(server), true)
                        if server.methods.contains(&method)
                            && names_the_plane_root(&method, &args) =>
                    {
                        Answered::Now(Some(reply(Outcome::Err(Value::String(
                            THE_PLANE_ROOT_IS_THE_WINDOWS.into(),
                        )))))
                    }
                    (Some(server), true) if server.methods.contains(&method) => {
                        let handler = Arc::clone(&server.handler);
                        Answered::Later(Box::pin(async move {
                            reply(match handler.call(method.clone(), args).await {
                                Ok(value) => Outcome::Ok(as_a_link_is_served(&method, value)),
                                Err(value) => Outcome::Err(value),
                            })
                        }))
                    }
                    (Some(_), true) => Answered::Now(Some(reply(Outcome::Err(Value::String(
                        format!("this host serves no UI RPC method `{method}`"),
                    ))))),
                    _ => Answered::Now(Some(reply(Outcome::Err(Value::String(
                        "the UI RPC is not open on this link: say hello first".into(),
                    ))))),
                }
            }
            // Only a host sends these.
            Frame::Welcome { .. } | Frame::Refused { .. } | Frame::Reply(_) => Answered::Now(None),
        }
    }

    fn hello(&mut self, build: &str, peer: &Peer) -> Frame {
        // A hello that is refused closes what an earlier one opened.
        self.open = false;
        let refused = |why: String| Frame::Refused { why };
        let Some(server) = &self.server else {
            return refused("this host serves no UI RPC".into());
        };
        if !peer.scope().may(Power::UiRpc) {
            return refused(format!(
                "the UI RPC is the app's window's alone (`local-ui`), and this link is `{}`",
                peer.scope().word()
            ));
        }
        if build != server.build {
            return refused(format!(
                "the UI RPC is private to one build: this host is `{}`, the window `{build}`; \
                 the session protocol carries on",
                server.build
            ));
        }
        self.open = true;
        Frame::Welcome {
            build: server.build.clone(),
        }
    }
}

/// What a [`Client`] asks the link's driver for.
pub(crate) enum Ask {
    Hello(String, oneshot::Sender<Result<(), CallError>>),
    Call(String, Value, Answering),
}

/// Where a call's answer goes: the command's value or its own error, or why none came.
type Answering = oneshot::Sender<Result<Result<Value, Value>, CallError>>;

/// The client driver's UI RPC: who waits for which answer.
#[derive(Default)]
pub(crate) struct Waiting {
    hellos: Vec<oneshot::Sender<Result<(), CallError>>>,
    calls: HashMap<u64, Answering>,
}

impl Waiting {
    /// The frame to send for `ask`, numbered `id`.
    pub(crate) fn ask(&mut self, id: u64, ask: Ask) -> Frame {
        match ask {
            Ask::Hello(build, reply) => {
                self.hellos.push(reply);
                Frame::Hello { build }
            }
            Ask::Call(method, args, reply) => {
                self.calls.insert(id, reply);
                Frame::Call(Call { id, method, args })
            }
        }
    }

    /// A frame the host sent.
    pub(crate) fn answered(&mut self, frame: Frame) {
        match frame {
            Frame::Welcome { .. } => {
                if !self.hellos.is_empty() {
                    let _ = self.hellos.remove(0).send(Ok(()));
                }
            }
            Frame::Refused { why } => {
                if !self.hellos.is_empty() {
                    let _ = self.hellos.remove(0).send(Err(CallError::Refused(
                        crate::session::Refusal {
                            code: crate::session::NOT_ALLOWED.into(),
                            why,
                        },
                    )));
                }
            }
            Frame::Reply(Reply { re, outcome }) => {
                if let Some(reply) = self.calls.remove(&re) {
                    let _ = reply.send(Ok(match outcome {
                        Outcome::Ok(value) => Ok(value),
                        Outcome::Err(value) => Err(value),
                    }));
                }
            }
            Frame::Hello { .. } | Frame::Call(_) => {}
        }
    }

    pub(crate) fn closed(&mut self, why: &str) {
        for reply in self.hellos.drain(..) {
            let _ = reply.send(Err(CallError::Closed(why.to_owned())));
        }
        for (_, reply) in self.calls.drain() {
            let _ = reply.send(Err(CallError::Closed(why.to_owned())));
        }
    }
}

/// The window's end of the UI RPC, once the host welcomed it.
#[derive(Clone)]
pub struct Client {
    asks: mpsc::UnboundedSender<LinkAsk>,
}

impl Client {
    pub(crate) async fn open(
        asks: mpsc::UnboundedSender<LinkAsk>,
        build: &str,
    ) -> Result<Client, CallError> {
        let (reply, welcomed) = oneshot::channel();
        asks.send(LinkAsk::Ui(Ask::Hello(build.to_owned(), reply)))
            .map_err(|_| CallError::Closed("the link is closed".into()))?;
        welcomed
            .await
            .map_err(|_| CallError::Closed("the link is closed".into()))??;
        Ok(Client { asks })
    }

    /// Call `method` with `args`: the command's value, or its own error.
    pub async fn call(&self, method: &str, args: Value) -> Result<Result<Value, Value>, CallError> {
        let (reply, answer) = oneshot::channel();
        self.asks
            .send(LinkAsk::Ui(Ask::Call(method.to_owned(), args, reply)))
            .map_err(|_| CallError::Closed("the link is closed".into()))?;
        answer
            .await
            .map_err(|_| CallError::Closed("the link is closed".into()))?
    }
}

#[cfg(test)]
mod credential_tests {
    use super::*;

    struct Nothing;

    impl Handler for Nothing {
        fn call(&self, _: String, _: Value) -> BoxFuture<'_, Result<Value, Value>> {
            Box::pin(async { Ok(Value::Null) })
        }
    }

    #[test]
    fn the_clipboards_text_is_never_among_the_methods_a_link_serves() {
        // ADR 0052 amended 2026-10-14: a host built with the app's whole command list still
        // refuses a link the read, so nothing that speaks over a link reads the clipboard.
        let server = Server::new("0.0.0+test", ["rename_chat", "clipboard_text"], Nothing);
        assert_eq!(
            server
                .methods
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["rename_chat"]
        );
        assert!(window_only("clipboard_text"));
        assert!(!served_on_a_link("clipboard_text"));
    }

    #[test]
    fn a_chats_browsers_are_listed_to_the_window_alone_and_never_on_a_link() {
        // #2024: the list is for watching, and a browser is watched in the person's window.
        let server = Server::new("0.0.0+test", ["rename_chat", "chat_browsers"], Nothing);
        assert_eq!(
            server
                .methods
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["rename_chat"]
        );
        assert!(window_only("chat_browsers"));
        assert!(!served_on_a_link("chat_browsers"));
    }

    #[test]
    fn a_vaults_sign_in_set_up_is_never_among_the_methods_a_link_serves() {
        // #1527: a host built with the app's whole command list still leaves them out.
        let server = Server::new(
            "0.0.0+test",
            WINDOW_ONLY_CREDENTIALS
                .iter()
                .chain(["rename_chat", "answer_ask"].iter())
                .copied(),
            Nothing,
        );
        assert_eq!(
            server
                .methods
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["rename_chat"]
        );
        for method in WINDOW_ONLY_CREDENTIALS {
            assert!(window_only(method), "{method}");
        }
        assert!(window_only("answer_ask"));
        assert!(!window_only("rename_chat"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_only_field_is_of_a_method_a_link_is_served_and_is_replaced_at_any_depth() {
        // A method of the window's alone is never served, so a field of it listed here would
        // say nothing.
        for kept in WINDOW_ONLY_FIELDS {
            assert!(!window_only(kept.method), "{}", kept.method);
        }
        let reply = serde_json::json!({
            "asks": [{"ask": "a", "writes": {"after": "x"}}, {"ask": "b"}],
            "nested": {"deeper": [{"writes": "y", "kept": 1}]},
            "none": {"writes": null}
        });
        let not_shown = an_edit_drawn_in_the_window_only();
        assert_eq!(
            as_a_link_is_served("pending_asks", reply.clone()),
            serde_json::json!({
                "asks": [{"ask": "a", "writes": not_shown}, {"ask": "b"}],
                "nested": {"deeper": [{"writes": not_shown, "kept": 1}]},
                "none": {"writes": null}
            })
        );
        assert_eq!(as_a_link_is_served("rename_chat", reply.clone()), reply);
    }

    /// Issue 1916: the folder watch is served on a link for a branch's folders, and refused
    /// there for a folder of the plane root, which is named by no workspace and no repo.
    #[test]
    fn a_folder_watch_that_names_the_plane_root_is_not_a_links_to_ask() {
        let folder = |workspace: &str, repo: &str| {
            serde_json::json!({
                "plane": "/p", "workspace": workspace, "repo": repo, "piece": null, "folder": "docs"
            })
        };
        let watch = |folders: Vec<Value>| serde_json::json!({ "folders": folders });
        assert!(!window_only("files_watch"));
        assert!(!names_the_plane_root("files_watch", &watch(vec![])));
        assert!(!names_the_plane_root(
            "files_watch",
            &watch(vec![folder("alpha", "svc")])
        ));
        assert!(names_the_plane_root(
            "files_watch",
            &watch(vec![folder("", "")])
        ));
        // One such folder among a branch's refuses the whole call.
        assert!(names_the_plane_root(
            "files_watch",
            &watch(vec![folder("alpha", "svc"), folder("", "")])
        ));
        // A folder that leaves the names out names none.
        assert!(names_the_plane_root(
            "files_watch",
            &watch(vec![serde_json::json!({ "plane": "/p", "folder": "" })])
        ));
        assert!(!names_the_plane_root(
            "branch_tree",
            &watch(vec![folder("", "")])
        ));
        // A call that does not say its folders as a list names no branch.
        for args in [
            serde_json::json!({}),
            serde_json::json!([[folder("alpha", "svc")]]),
            serde_json::json!({ "folders": folder("alpha", "svc") }),
            Value::Null,
        ] {
            assert!(names_the_plane_root("files_watch", &args), "{args}");
        }
    }

    #[test]
    fn a_host_serves_only_what_the_served_list_names_whatever_it_is_built_with() {
        // #1927: a command on neither list (one added to the app and not yet listed) is
        // refused to a link at the host, not only by a test of the lists.
        struct Nothing;
        impl Handler for Nothing {
            fn call(&self, _: String, _: Value) -> BoxFuture<'_, Result<Value, Value>> {
                Box::pin(async { Ok(Value::Null) })
            }
        }
        let server = Server::new(
            "0.0.0+test",
            [
                "rename_chat",
                "a_command_added_later",
                "send_input",
                "answer_ask",
            ],
            Nothing,
        );
        assert_eq!(
            server
                .methods
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["rename_chat"]
        );
    }

    #[test]
    fn no_command_is_both_served_on_a_link_and_the_windows_alone() {
        // #1927: the two lists part the app's commands. The app's own test holds that every
        // command is on one; this holds that none is on both, and each list names one once.
        for command in SERVED_ON_A_LINK {
            assert!(!window_only(command), "{command} is on both lists");
            assert!(served_on_a_link(command), "{command}");
        }
        for command in window_only_commands() {
            assert!(!served_on_a_link(command), "{command}");
        }
        let mut once = SERVED_ON_A_LINK.to_vec();
        once.sort_unstable();
        once.dedup();
        assert_eq!(
            once.len(),
            SERVED_ON_A_LINK.len(),
            "a command is listed twice"
        );
    }

    #[test]
    fn every_command_of_who_may_dispatch_to_whom_is_on_the_window_only_list() {
        for command in STANDING_DISPATCH {
            assert!(WINDOW_ONLY.contains(command), "{command}");
        }
        // Named once each.
        let mut sorted = STANDING_DISPATCH.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), STANDING_DISPATCH.len());
    }
}
