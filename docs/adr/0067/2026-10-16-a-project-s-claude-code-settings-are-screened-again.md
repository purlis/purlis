# Amended (2026-10-16; extends 2026-10-15-05): a project's Claude Code settings are screened again when they change

Decided by the dispatcher under delegation; the operator can veto.

[2026-10-15-05](2026-10-15-05-a-sandboxed-claude-code-chat-takes-its-settings-from.md) decides at
each start whether a sandboxed Claude Code chat loads its project's own settings files. The
decision now holds for as long as the chat runs:

- **Watched from before the start.** For a sandboxed Claude Code chat that loads its project's
  files, purlis watches the folders those files are in, and the folder above each, before the
  chat starts. A `.claude` folder made later is watched from then on.
- **Screened again on every change.** A change from any source, such as the window's own pull or
  save or the person's editor, is screened as a start screens it.
- **A file that still passes changes nothing.**
- **A file that no longer passes restarts the chat, from the core.** The core starts the chat
  again on its conversation at once, at the size its terminal was last given, whether or not a
  window is open. A permission ask of the old run's is refused, and a stop of it gives way. The
  new run starts on the person's settings and purlis's, with the project's deny and ask rules.
  Its tab shows the same Notice a start shows, naming the keys that were not applied. The chat
  is told, as it starts again, that it was restarted because the project's settings changed.
  The window only redraws.
- **The old run ends before the new one starts**, so two runs never share a conversation.
- **A chat the person is stopping is ended**, as the stop would have ended it, and not started
  again.
- **A chat that cannot be started again is ended.** Where there is no conversation to resume,
  or the start is refused, the old run is ended. It is kept, with why, among the chats that
  would not start, so a window that opens later shows it with a Retry.
- **A change caught as the chat starts is the start's own.** The first screen after its program
  starts answers the start. On a change, that run is ended and the chat is started once more on
  the person's settings and purlis's, and the start answers that run, the one that goes on.
- **Fail closed.** Where the watch cannot be set up, the chat starts on the person's settings and
  purlis's from the first, and its tab says so.
- A chat started on the person's settings is not watched. A later change does not move it back
  to the project's files until it starts again.

## Decisions

- **D-PR-1: restarted at once, by the core.** A restart a grant owes waits for the chat's turn
  to end, so a running build is not cut off by an Allow. This restart keeps the chat on what its
  start decided, so it neither waits for a pause nor depends on a window to drive it.
- **D-PR-2: every watched chat is screened again on a change to a settings folder or file.**
  Changes are those to `.claude`, `settings.json` or `settings.local.json`, in any case. Reads
  and opens are not changes. Each screen reads a few small files. An event may name a folder by
  another spelling than the chat's (a link, a case-folding disk), so no event is matched to one
  chat by its path.
- **D-PR-3: the watcher's own thread only says something changed.** Another thread screens and
  adds the watch on a new `.claude` folder, since adding a watch from the watcher's own thread
  waits on that same thread.
- **D-PR-4: a run of changes is waited out briefly before the files are screened.** The files
  are screened once they have been quiet for 150 ms, and after 500 ms at most. A file being saved
  is often seen empty or half written for a moment, and that is not what it holds.
- **D-PR-5: the watch survives a folder made again.** A folder that is taken away and made again
  is listened to again: on Linux at every screen, where inotify takes a watch already standing
  as a no-op, and elsewhere when the folder is not the one it was.
