# Amended 2026-11-02: a start and a dispatch name where their chat runs

**Built by #4536** (parts 2 and 3), on the amendment of 2026-11-01 that ordered the levels and the
person's ruling of the same day. Points 1 to 3 are that ruling as built; the rest is *decided in
implementation, to be confirmed* (the D-lines of the pull request that built it).

1. **Start chat has a Runs on row.** It says where the new chat runs and which level says so
   (the workspace, the project, or this machine when nothing names a pool), and offers this
   machine or any pool this machine keeps, for that start alone. The choice never writes a
   setting. A chat at the project's root runs here and is offered no pool.
2. **A pool that is gone, stopped or not reached starts nothing.** The start says why and offers
   Try again, Pool setup, and Run on this machine this once. Only that last press runs it here,
   and only that once. It is decided by this machine's own findings (the pool isn't kept with its
   runner, or the link to it didn't come up for the start), never by a runner's words.
3. **A dispatch names a pool or this machine** (`purlis dispatch --pool <name>` or
   `--this-machine`, and the dispatch tool's `pool` and `this_machine`), in the same order, for
   its target workspace. A chat on a pool's runner that dispatches anything, this machine among
   it, is held for the person as the amendment of 2026-10-30-07 says, and the held ask's tab says
   where the task would run before Approve.
4. **The choice is kept with the chat**, in `reopen.json` (`chats[].runs_on`), so a relaunch or
   a restart starts it where it ran: its conversation is there. A one-off start means a
   workspace can have chats in two places while they share its clones; a move still takes every
   chat of its workspace running where it runs (2026-10-29).
5. **A word a dispatch carries that purlis can't read is refused**, never started where the
   levels say instead.
