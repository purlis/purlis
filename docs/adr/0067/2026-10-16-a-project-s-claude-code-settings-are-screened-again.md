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
- **A file that no longer passes restarts the chat.** The chat is owed a restart on its
  conversation, and the window drives that restart at once, as Restart now does. The new run
  starts on the person's settings and purlis's, with the project's deny and ask rules. Its tab
  shows the same Notice a start shows, naming the keys that were not applied. The chat is told,
  as it starts again, that it was restarted because the project's settings changed.
- **Fail closed.** Where the watch cannot be set up, the chat starts on the person's settings and
  purlis's from the first, and its tab says so.
- A chat started on the person's settings is not watched. A later change does not move it back
  to the project's files until it starts again.

## Decisions

- **D-PR-1: restarted at once, not at the next pause.** A restart a grant owes waits for the
  chat's turn to end, so a running build is not cut off by an Allow. Here the restart keeps the
  chat on what its start decided, so it is not left waiting.
- **D-PR-2: every watched chat is screened again on any change in the folders watched.** Each
  screen reads a few small files. An event may name a folder by another spelling than the chat's
  (a link, a case-folding disk), so no event is matched to one chat by its path.
- **D-PR-3: the watcher's own thread only says something changed.** Another thread screens and
  adds the watch on a new `.claude` folder, since adding a watch from the watcher's own thread
  waits on that same thread.
