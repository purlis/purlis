# How a restart leaves a chat idle with nothing to wake it

Research, 2026-11-08. Facts only. This document decides no policy; the person rules on it
afterwards. Read from `main` at `e5b269c03`.

**The bug.** A chat's turn can end while background work of its own is still in flight. When the
app restarts, that work is lost. The resumed harness never gets the work's notification, so it
waits for ever. The known instance waited about 45 minutes on a run marked `[killed]` (reported
by the dispatching chat on 2026-11-05, filed against #5253's work; not reproduced here).

**Marks.** Every cell carries one:

- **V** (verified from code): read at the cited `file:line` in this tree.
- **R** (verified from a run): seen on this machine, from Claude Code's own files or a tool's own
  description in a live session.
- **I** (inferred): follows from the verified facts but was not read or seen directly.

Paths are relative to the repo root.

## Prior work, and what each covers

| Ticket | What it covers | What it leaves out |
|---|---|---|
| #3878 (closed) | The board state **waiting on its own work**. Claude Code's `Stop` `background_tasks` (`subagent`, `workflow`, `monitor`, `shell`) and `session_crons` are read into `Said` (`crates/purlis-core/src/harness/claude/ownwork.rs:41-85`). The app stamps it on its own clock (`crates/purlis-core/src/ownwork.rs:112-125`). A shell or a recurring cron counts for 60 minutes (`ownwork.rs:35`); a wake-up counts until 5 minutes past due (`ownwork.rs:40`). Exposed as `Board::own_work` (`crates/purlis-core/src/state.rs:1861`) and `Glance::in_its_turn` (`app/src-tauri/src/host.rs:857-859`). | Restarts. Codex and opencode. |
| #3887 (closed) | A move to a runner, a sandbox Allow's restart and a repo save wait while the chat waits on its own work (`app/src-tauri/src/chatmoves.rs:992-998`, `app/src/chatState.ts:479-495`, `app/src-tauri/src/planes.rs:1947-1972`). | A restart purlis cannot avoid: quit, update, crash. A restart made "now". Anything after the 60-minute lapse. |
| #3897 / #3900 (closed) | Dispatched tasks across a restart: liveness in `dispatch list` and the roster, and **orphaned by restart** (`crates/purlis-core/src/dispatchliveness.rs:100,147-149`). | A chat's own background work. An idle asker's wake. |
| #1513 (closed) / #1546 (closed, items unticked) | Tasks resume on their conversation with `CARRY_ON` (`crates/purlis-core/src/dispatchrestart.rs:70-75`). Reports are kept on disk and never lost. #1546 records as open items that "a report handed to a chat that came back waits for its next turn; no line is typed into it" and that "the asking chat brought back at a launch is told nothing about its restored tasks". | A chat's own background work. |
| #666 (closed) | The supervisor restarts the session host, with a breaker (`app/src-tauri/src/sessionhost.rs:19-25`). `Cause::HostCrash` exists (`crates/purlis-core/src/state/run.rs:173,200,599-621`). | Re-attaching harness-native hosts (FD-20): no code found. Own work is not consulted when a host crash is read (below). |
| #5170 (closed) | A restart that finds no conversation id is refused (`app/src-tauri/src/chats.rs:1285-1291`). The fix keeps an id named before the chat is listed (`chats.rs:993-1000`). | Own work. |
| Also: #4647 (closed) | A late wake after a lapsed wait is a background turn, not the person's reply (`own_work_out`, `state.rs:634`). | Restarts. |
| Also: #665 (open), FD-28 | Upgrading without losing agents. | Open, so an update ends every chat today (below). |
| Also: #1391 (open), SC-4 | Hibernating idle chats. | Open. |

## What each kind of restart does to a chat

This table is the base the matrix below reads from.

| Restart | Harness process and its child processes | purlis's record of own work | What purlis sends the resumed chat |
|---|---|---|---|
| **Quit and relaunch** | Ended. `RunEvent::Exit` → `let_go_of_every_plane` → `Held::let_go` → `Chats::end_all` (`app/src-tauri/src/planes.rs:1921-1935`, `chats.rs:8177-8179`). The host is told to "end every chat as a quit" (`sessionhost.rs:462-476`). **V** | Lost. `reopen.json` holds the conversation id (`crates/purlis-core/src/reopen.rs:71-79`) and has no own-work field. The board lives only in the host's memory (`crates/purlis-core/src/state/kept.rs:4-13`). **V** | The harness is resumed by id: `claude --resume {id}`, `codex resume {id}`, `opencode -s {id}` (`crates/purlis-core/src/harness_declaration/{claude,codex,opencode}.toml`). No line is sent, except `CARRY_ON` to a dispatched task that still owes its report (`dispatchrestart.rs:96-99`). **V** |
| **App update** | Ended, exactly as at a quit. `let_go_of_all_to_update_choosing` runs the same `let_go_of_every_plane(true)` → `end_all` (`planes.rs:5316-5317,1929-1935`); only `relaunch_after_update` differs. ADR 0068's table says an update "keeps every session"; that waits on #665, which is open. **V** | Lost, as at a quit. **V** | As at a quit. **V** |
| **Window crash, back within 10 min** | Kept. The host keeps every program for `GRACE` (`crates/host/src/lifetime.rs:6-10,31`). Exception: a host started `ends_with_its_window` ends them at once (`crates/host/src/hosting.rs:981-984`). **V** | Kept. The window takes the host's board whole and re-arms the lapse timer (`app/src-tauri/src/hooks.rs:700-756`). **V** | Nothing: the program never stopped. **V** |
| **Window crash, back after 10 min** | Ended "as a quit would" (`lifetime.rs:11-16`). **V** | Lost. **V** | As at a quit. **V** |
| **Session host crash** | Ended with the host (ADR 0068 §3). **V** | Not consulted. `after_the_host(state, asking, …)` reads only state and asking (`app/src-tauri/src/hostlost.rs:62-84`). A chat in `Waiting` with own work maps to `InputRequired(TurnEnded)` → `Hibernated`. **V** | The pane says the host "stopped unexpectedly while {chat} was waiting for you, so nothing was lost" (`app/src/HostLostNotice.tsx:49-50`). Resume is the ordinary restart by id, and nothing more is sent. **V** |
| **Move to a runner** | Ended, then started again on its conversation (`chatmoves.rs:978-1012`). It waits while `in_its_turn()`, own work included (`chatmoves.rs:992-998`). **V** | After the 60-minute lapse, `own_work` is `None` (`state.rs:1270-1276`) and the move goes ahead. `own_work_out` is not read there. **V** | Nothing about own work. **V** |
| **Sandbox Allow's restart** | Restarted by id. The window waits for `atAPause`, which excludes own work (`chatState.ts:479-495`). The core's `Held::restart_chat` checks only open asks and stopping, not own work (`planes.rs:1586-1598`). Bypasses that restart at once: **Restart now** (`app/src/PlaneView.tsx:7780-7785`), `restart_without_sandbox` ("mid-turn or not", `chats.rs:3640-3644`), `contain_project_settings` (`planes.rs:1610-1616`). **V** | Lost: `Session::Began` clears `own_work` (`state.rs:1069-1074`). **V** | The owed lines, e.g. a port taken back (`chats.rs:3127-3133,3611`). Nothing about own work. **V** |
| **Restart after a skill change** | The same window driver as the sandbox restart (`app/src/SkillsChangedNotice.tsx:20-24` → `askRestart`), so it waits for `atAPause`. **V** | As for the sandbox restart. **V** | Nothing. **V** |
| **Repo save** | Not a restart. A save is refused while `mid_turn_in`, which counts own work (`planes.rs:1947-1972`, `crates/purlis-core/src/reposave.rs:101-112`). Auto-save skips the cycle (`app/src-tauri/src/autosave.rs:523-528`). **V** | Unchanged. **V** | Nothing. **V** |
| **Mac sleep and wake** | No sleep or wake handling: no `NSWorkspace` power observer and no IOKit notification (`NSWorkspace` appears only in `app/src-tauri/src/syscontrast.rs:48-49`). **V** Processes are frozen and resume with the machine. **I** | Kept. The lapse thread reads the wall clock every 30 s, "so a wait that ran out while the machine slept ends as it wakes" (`hooks.rs:3802-3805`). **V** | Nothing. **V** |

Two warnings do not count own work:

- The quit warning and the update dialog list a chat as mid-turn only when `state === "running" || "unknown"` (`app/src/QuitWarning.tsx:129-130`, `app/src/Updates.tsx:295`). A chat waiting on its own work says `waiting`, so neither lists it. **V**
- `mid_turn_everywhere` at quit does count it, but only to journal the repo save as skipped (`planes.rs:1970`). **V**

## Facts about the harnesses

- **Own work is read for Claude Code only.**
  - `hookwire.rs:265-274` calls `claude::ownwork::read` only when the chat's harness is `claude-code` (`BACKGROUND_TASKS_HARNESS`, `hookwire.rs:377-380`). ADR 0073's amendment `docs/adr/0073/2026-10-29-a-turn-may-end-waiting-on-its-own-work.md:13-16` says the same. **V**
  - A Codex or opencode `Stop` is `Turn::Ended` (`state.rs:352-358`), so its turn's end always goes to the person. **V**
- **Codex.** `SubagentStop` is armed, and a child only shows as done (`crates/purlis-core/src/harness/codex.rs:121-125`, `state/children.rs:12-13`). A command still running when its call returned is read for sandbox blocks only (`crates/purlis-core/docs/hooks.md:36`). The declaration says Codex never reports that it waits on anything but an approval (`codex.toml:50`). **V**
- **opencode.** `session.idle` becomes a `Stop` that carries only the session id, cwd and last message (`app/src-tauri/plugin/opencode/purlis.ts:475-486`). Sub-agent sessions are skipped (`purlis.ts:477`). **V**
- **Claude Code `CronCreate`.** Its own tool description, in a live 2026-11-08 session, says: "Jobs live only in this Claude session — nothing is written to disk, and the job is gone when Claude exits." `durable` "has no effect". Recurring jobs expire after 7 days. **R**
- **Claude Code `Monitor`.** A monitor expires after at most 30 minutes by itself (its tool description). **R**
- **Claude Code background output on disk.** Each background shell or agent writes `/private/tmp/claude-<uid>/<project>/<session>/tasks/<id>.output`. This machine holds 4,380 such files, some from sessions days old, so they outlive the process. Sub-agent transcripts also land in `~/.claude/projects/<project>/<session>/subagents/agent-*.jsonl`. **R** macOS empties `/private/tmp` at a reboot. **I**
- **purlis reads Claude Code's `<tasks>/<id>.output` only to find sandbox blocks** (`crates/purlis-core/src/sandboxblock_background.rs:7,39-41`). Nothing in the tree mentions `scheduled_tasks.json`, `ScheduleWakeup`, `CronCreate` or `Monitor`, and nothing reads background work back after a resume. **V**
- **Whether `ScheduleWakeup` and `/loop` appear in `session_crons`.** The reader takes any `session_crons` entry (`claude/ownwork.rs:65-83`); whether those two are listed there was not checked. **I**
- **What a resumed Claude Code process knows of a background task its earlier process started.** Not reproduced. The known instance reports a run shown as `[killed]` and no notification. A kill inside a live process does deliver a `<task-notification>` with `<status>killed</status>`: seen 2026-11-02 in a transcript, when memory pressure stopped a watcher and the chat woke on it. **R** That is not a restart.

## The matrix: Claude Code

Restarts in three groups, from the table above:

- **Kills** (the harness process ends): quit and relaunch, app update, window crash past 10 minutes, session host crash, a sandbox or skill restart made "now", and a move or restart after the 60-minute lapse.
- **Waits** (purlis holds the restart while own work is set): move to a runner, sandbox Allow, skill change, repo save.
- **Survives** (the harness is untouched): window crash within 10 minutes; Mac sleep.

| Own work | Restart | What purlis knows today, and where | What the resumed chat sees today | Output survives on disk |
|---|---|---|---|---|
| `run_in_background` Bash shell | Kills | Before the restart: `shells: n`, held 60 minutes (`claude/ownwork.rs:60`, `ownwork.rs:173-175`). **V** After: nothing; the board is fresh or cleared (`state.rs:1069-1074`). **V** The shell is a child of the ended harness and ends with it. **I** | Its transcript, ending at the tool result that started the shell and the turn's last message, and no notification. purlis sends nothing (no own-work line anywhere; `dispatchrestart.rs:70-75` and `chats.rs:3611` are the only lines sent). **V** for purlis, **I** for the harness. The board shows `waiting` with no own work, and it never lapses to ✋ because there was no turn end to stamp (`state.rs:1069-1074`). **I** | Yes, partly: `<tasks>/<id>.output` up to the kill. **R** |
| Shell | Waits | `in_its_turn` holds the restart (`host.rs:857-859`). After 60 minutes `own_work` lapses (`state.rs:1270-1276`) and the restart goes ahead, though `own_work_out` records that work may still run. **V** | Within 60 minutes: no restart, so no loss. **V** After: as for Kills. **I** | As above. **R** |
| Shell | Survives | Kept in the host's board and re-armed (`hooks.rs:742-755`). **V** | The ordinary notification from the live harness. **I** | Yes. **R** |
| `Monitor` | Kills | `monitors: n`, no limit (`ownwork.rs:185`). **V** After: nothing. **V** | As for the shell. **I** | Yes, partly: its output file. **R** (same `tasks/` folder) |
| Monitor | Waits | Holds with no limit while it is in flight. **V** Claude Code itself ends a monitor within 30 minutes (**R**), which then comes back as a turn. **I** | No loss while it holds. **V** | — |
| Monitor | Survives | Kept. **V** | Notifications as usual. **I** | Yes. **R** |
| Background sub-agent | Kills | `helpers: n`, no limit (`claude/ownwork.rs:57`). **V** After: nothing. **V** The agent runs inside the Claude Code process and ends with it. **I** | Its transcript, ending where it handed the agent off. No result and no notification. **I** | Yes, partly: `subagents/agent-*.jsonl` and `<tasks>/<id>.output`. **R** |
| Sub-agent | Waits / Survives | As Monitor. **V** | As Monitor. **I** | Yes. **R** |
| `Workflow` run | Kills | `workflows: n`, no limit (`claude/ownwork.rs:58`). **V** After: nothing. **V** | As for a sub-agent. **I** | Not checked. Each agent's transcript likely survives, as a sub-agent's does. **I** |
| Workflow | Waits / Survives | As Monitor. **V** | As Monitor. **I** | — |
| `CronCreate` (session cron) | Kills | `wakes_in` for a one-shot; for a recurring cron, only when its next run is within 60 minutes (`ownwork.rs:177-184`). The lapse is due + 5 minutes. **V** After: nothing. **V** | No wake ever comes: the job lived only in the ended process. **R** (tool description) | **No**: "nothing is written to disk". **R** |
| Cron | Waits | Holds until due + 5 minutes. A recurring cron more than 60 minutes out does not hold, so a restart proceeds and drops it (`ownwork.rs:179`). **V** | After a restart, the cron is gone. **R** | No. **R** |
| Cron | Survives | Kept. **V** | Fires as usual. **I** | — |
| `ScheduleWakeup` / `/loop` | Kills / Waits / Survives | As for a cron, if it is listed in `session_crons`. **I** If it is not listed, nothing holds the restart. **I** | Not checked whether a resumed process re-arms it. **I** | Not checked. **I** |
| purlis `dispatch_wait` | Kills | This is a tool call mid-turn, not own work. It blocks on the hook socket for up to 540 s (`crates/purlis-cli/src/dispatch.rs:1561`, `crates/purlis-core/src/dispatched.rs:349,353`). The call dies with the harness. **V** | The interrupted turn. After a relaunch, a wait on a task that reported before the quit answers "The app no longer holds its text" (`dispatch.rs:1622-1624`), even when the report is still unread on disk. **V** | The report itself, yes: under `.charter/handbacks/chat-<n>/` (`crates/purlis-core/src/handback.rs:20-25`). **V** |
| `dispatch_wait` | Window crash within grace | The host drops asks waiting on the window, which are answered "purlis's window is not there to take this" (`crates/purlis-core/src/hookwire/relay.rs:267,679`). **V** for the text, **I** for the chain from `let_go` to it. | That refusal as the tool's result. **I** | Yes. **V** |
| A dispatched task's report, asker idle | Kills | The report is on disk; the asker is shown waiting (`Waits`, `state.rs:407-443`). The in-memory `landed` record, which decides whether a line is typed (`app/src-tauri/src/dispatched.rs:2975-3010`), is lost. `Waits` is not saved and comes back only when the asker's next turn ends (`state.rs:1069-1074`; nothing recomputes it at launch). **V** | Resumed tasks get `CARRY_ON`; the asker is sent nothing (`dispatchrestart.rs:96-99`). A report that lands after the relaunch is typed in as usual (`dispatched.rs:2975-3003`). **V** A report already pending at the quit, or handed back by `dispatchrestart::hand_to`, waits for the asker's next turn with no line typed (`app/src-tauri/src/restored.rs:547-598`). **V** That is the idle-for-ever case when nothing else prompts the asker. **I** | Yes: handback files and the dispatch record's `Undelivered` flag. **V** |
| Task report, asker idle | Window crash within grace | `landed` lives in the window's ledger. Whether a report landing while no window is attached gets a typed line once a window returns was not checked. **I** | Not checked. **I** | Yes. **V** |
| Task report, asker idle | A task that cannot come back | `end_at_launch` writes "ended without a report" and puts it in `landed`, so a line **is** typed (`restored.rs:291-299`, `dispatchrestart.rs:342-435`). **V** | A typed line. **V** | Yes. **V** |

## The matrix: Codex and opencode

| Own work | Restart | What purlis knows today, and where | What the resumed chat sees today | Output survives on disk |
|---|---|---|---|---|
| Any background work (Codex sub-agents, opencode sub-agent sessions, background commands) | Any | Nothing. Own work is not read, and its turn's end goes to the person (`hookwire.rs:265-274,377-380`; `state.rs:352-358`). **V** Codex sub-agents show only as children (`state/children.rs:12-13`). **V** | Before the restart, the chat already shows ✋, so a parent or the person sees it as theirs. **V** After resume by id (`codex resume {id}`, `opencode -s {id}`), nothing is sent. **V** What the harness itself restores was not checked. **I** | Not checked. **I** |
| A Codex or opencode chat never prompted | Kills | No conversation id: Codex names it only inside its first turn (`crates/purlis-core/src/harness.rs:207-211`). At relaunch it starts fresh (`reopen.rs:937-950`); a restart is refused with `no_conversation` (`chats.rs:1285-1291`, #5170). **V** | A new conversation. **V** | — |
| `dispatch_wait` / task reports | Any | As for Claude Code: the dispatch path does not depend on the harness, except that a typed line needs one of the three (`told_by_a_line`, `crates/purlis-core/src/dispatched.rs:1364`). **V** | As for Claude Code. **V** | Yes. **V** |

Remote (vendor-run) chats exist only as the `Kind::Observed` run model (`state/run.rs:227-233,634-641`). No own-work state was found for them. **V** Runner chats use the Claude Code path: the hook runs on the runner and sends durations (`ownwork.rs:24-28`), and this machine's host keeps their board (`app/src-tauri/src/hostedsessions.rs:3861-3863`). Chats on a runner keep running while the desktop is away (`lifetime.rs:23-25`). **V**

## Timing cases

| Case | Kills | Waits | Survives |
|---|---|---|---|
| **The work finished during the downtime, and its notification was lost** | With Claude Code, the work is ended with the harness, so it does not finish (**I**). Two exceptions: work detached from the process tree (`nohup`, `setsid`, a CI run a Monitor polled), and a dispatched task that ran on. The detached work finishes unseen; its output file stops at the kill (**R** for the file). A task's report is kept on disk and waits for the asker's next turn with no typed line when it was pending at the quit (`restored.rs:547-598`, **V**). | Not reached while held. After the 60-minute shell lapse, a shell that finishes later wakes a live chat as a background turn (#4647, `own_work_out`, **V**), unless a restart has gone ahead in between (**I**). | Window crash: the harness is alive and gets its notification (**I**). The host hears its hooks while no window is attached (`kept.rs:10-11`, **V**), but no lapse fires until a window takes the board back (`hooks.rs:742-755`, **V**). Sleep: the work is frozen and finishes after wake (**I**). |
| **A wake-up or cron came due during the downtime** | The job is gone with the process (**R**, `CronCreate`). Nothing purlis keeps knows it was due (**V**). | A one-shot holds the restart until due + 5 minutes (`ownwork.rs:40,183`, **V**), so it fires first. A recurring cron more than 60 minutes out does not hold (`ownwork.rs:179`, **V**) and is dropped by the restart (**R**). | Window crash: it fires in the live harness (**I**). Sleep: Claude Code fires a due job at its next idle check (`ownwork.rs:37-39` says so of the harness, **V** for the comment). Whether it fires once or for each missed match after a long sleep was not checked (**I**). The board's wait lapses on wake if more than due + 5 minutes has passed (`hooks.rs:3802-3805`, **V**), so the chat may show ✋ a moment before the wake (**I**). |
| **The chat was mid-turn, not idle** | Own work is recorded only from a `Stop` (`state.rs:352`), so background work started in the cut turn was never on the board (**V**). The quit warning and the update dialog list the chat as mid-turn (`QuitWarning.tsx:129-130`, **V**). The resumed harness sees an interrupted turn, and purlis sends nothing but `CARRY_ON` to tasks (**V**). | A running chat is `in_its_turn` (`host.rs:858`), so it is held (**V**). The bypasses (Restart now, without sandbox, project settings) cut it (`chats.rs:3640-3644`, **V**). | Untouched (**V**). A `dispatch_wait` in flight during a window crash is refused (`relay.rs:267`, **I** for the chain). |

## Cases no existing ticket covers

Each case was checked against the tickets above and searched on GitHub (`restart background`,
`own work restart`, `sleep wake`, `quit warning`, `nothing was lost`). "Partly" names the ticket
that touches it.

1. **A kill (quit, update, window crash past 10 minutes, host crash) ends a Claude Code chat's
   own background work, and nothing tells the resumed chat.** It sits idle with no notification,
   no lapse to ✋, and nothing in `reopen.json` to say it was waiting. This is the bug itself. #3887
   chose to do "nothing after a restart that couldn't be avoided".
2. **An app update ends every chat as a quit does** (`planes.rs:5316`). ADR 0068's table says it
   keeps them. Partly #665 (open), and #1974's story 10 asks the update to say so.
3. **The quit warning and the update dialog do not list a chat waiting on its own work** as one
   the restart will cut (`QuitWarning.tsx:129-130`, `Updates.tsx:295`).
4. **After a host crash, a chat waiting on its own work is told "nothing was lost"**
   (`hostlost.rs:68-74`, `HostLostNotice.tsx:49-50`), though its background work was killed.
5. **Own work is not persisted anywhere** that outlives the host: not in `reopen.json`, not in
   the host record (`crates/host/src/hostrecord.rs:17-25`). Its output files do survive (`tasks/`),
   and the `Stop` lines are in the event log (`crates/purlis-core/src/hookchannel/record.rs:4-18`);
   nothing reads either back at launch.
6. **The core's `restart_chat` does not check own work** (`planes.rs:1586-1598`). Only the
   window's driver waits. **Restart now**, **restart without sandbox** and **contain project
   settings** cut own work at once.
7. **After the 60-minute shell lapse, a move or restart goes ahead and cuts a shell that is
   still running.** `own_work_out` (#4647) is not read by the pause rules (`chatmoves.rs:992-998`,
   `chatState.ts:479-495`).
8. **A recurring cron whose next run is more than 60 minutes out does not hold a restart**, and
   the restart drops it, because `CronCreate` jobs live in memory only.
9. **Mac sleep and wake: no handling.** Nothing checks after wake whether the harness's
   background work survived, and the wake-up lapse can raise ✋ just before the wake fires. Partly
   #1391 (hibernate, open), which is about putting chats to sleep on purpose.
10. **Background work started in a turn that a kill cut mid-turn is never recorded**, because own
    work is read only from a `Stop`.
11. **An asker's `Waits` (held, not ✋) is not restored at launch.** It comes back only at its next
    turn's end, so after a relaunch an idle asker of live tasks has no wait shown and no prompt
    coming. Partly #1546 ("the asking chat brought back at a launch is told nothing about its
    restored tasks", closed with the item unticked).
12. **A report pending at the quit, or handed back to a returning asker, is never typed in**: it
    waits for a next turn that nothing starts. Covered as an unticked item of #1546 (closed) and
    of #1463 (open: "the ledger is in memory … lost on an app relaunch"); neither is a ticket of
    its own.
13. **`dispatch wait` on a task that reported before the quit says the app "no longer holds its
    text"** (`dispatch.rs:1622-1624`), though the report is on disk. Partly #1463.
14. **Codex and opencode chats with background work show ✋ while it runs** and are not told at a
    restart. This is ADR 0073's decision for those harnesses, not a gap in a ticket; listed so the
    ruling can include them.
15. **`ScheduleWakeup` and `/loop`**: whether Claude Code lists them in `session_crons`, so that
    they hold a restart at all, is unverified. No ticket.

Not uncovered, for the record: the waits on a move, a sandbox Allow, a skill restart and a save
(#3887); task liveness and orphans (#3900); tasks resumed with `CARRY_ON` (#1513); the
no-conversation refusal (#5170); a late wake after a lapse (#4647).
