# Amended 2026-11-10: Quiet is built, and which Expiring asks it can hold open

Decision 13 is built (#6017) in the notifier (`app/src-tauri/src/asknotify.rs`), with these
particulars.

- **Three sources, one Quiet.** The schedule (today's quiet hours, read from the layout file),
  "Quiet now" (a switch at the top of Asks and a palette row, held in memory until turned off or
  the app ends), and the person typing: any key a chat's pane sends, except the terminal answering
  its program by itself, holds Quiet until `TYPING_QUIET` (30 s) after the last one. Nothing new is
  stored, so no tier is named (ADR 0069).
- **macOS's own Focus** holds purlis's notifications as it holds every app's: the system does it,
  and purlis does not read Focus. Reading it takes an entitlement purlis does not have.
- **The summary says "While you were quiet"**, not "While you were focused": the glossary keeps
  focus mode for #2921. The core sends each window what Quiet held (its span, and the asks that came
  in it that still wait); the window counts the updates of the same span ("2 finished, 1 failed")
  and draws the row from what is still New (#6002). The row goes once all of it is seen, or is
  dismissed.
- **One notification as Quiet ends, only if the person is away** from every window of the app,
  naming the chats still waiting: "While you were quiet: 3 chats need you". At a window, nothing is
  sent, and the row in Asks says it.

## Expiring asks, held open past their time

None is held open yet; that is left to #6007. What ends each one:

| Ask | Its time | What ends it | Can Quiet hold it open? |
| --- | --- | --- | --- |
| Sandbox host consent (a live, held connection) | 60 s, `sandbox::asks::HOLD` | purlis's own timer, in the session host's proxy (`Asks::hold`) | **Yes, with a protocol change**: the session host must be told Quiet (a new request, or an `askable`-like closure fed over the link), and the window's `hold_ends` must match. Still bounded by the program's own connect timeout and the harness's command timeout. |
| Permission prompt (`PermissionRequest` hook) | about 58 s | the harness's hook timeout, 60 s, which purlis writes into the harness's config at the chat's start | **No** for an ask already open: Claude Code and Codex kill the hook. Only a longer timeout written at the chat's start would. At the end the harness's own prompt decides in the pane. ACP asks have no deadline. |
| Run grant (`purlis secret exec`) | 90 s (7.5 s on Codex) | three quarters of the harness's command timeout (120 s; 10 s on Codex) | **No**: the harness kills the command. It already ends "still open, answer in Asks", answerable after. |
| Secret ask | as a run grant | as a run grant, or the harness's MCP tool timeout | **No**, as above. |
| Browser login, secret fill, pool tool | 15 min | purlis's own timer, bounded by the waiting `purlis` command's own read deadline and the harness's tool or command timeout | Not as they are: the waiting command gives up first. |
