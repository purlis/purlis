# Amendment, 2026-11-07: a move cut short is finished by the next

Decision 4's *all or nothing* held only while purlis kept running: what to put back lived in
memory. Four gaps from #5374's review are closed here (#5569). The rest of the decision stands.

- **A move cut short by a crash, a kill or a power cut is finished, not rolled back.** Before a
  workspace's first clone moves, a journal names it and its clones
  (`.charter/move-into-repos.json`, Clone state, transient); it is removed once the move is done
  or put back. One still there means some clones are in `repos/` with their branch folders not
  repaired. `doctor` has a row for it, *move into repos/ cut short*, whose fix is the same
  `move-into-repos`, and that fix first runs `git worktree repair` in each clone the journal
  names that is in `repos/`, then moves what is left. Finishing is the smaller step: both
  layouts are read for ever, so a clone already in `repos/` is where it belongs, and moving it
  back would be a second move to get right after a crash.
- **An open chat the app cannot place still holds the move back.** A chat with no folder could
  be in any workspace, so it holds back every one; a chat with no name to show is named by its
  own. The chats are asked again once a workspace's clones have moved, and one opened meanwhile
  puts everything back.
- **`Foo/` beside `repos/foo/` on a disk that ignores case is *beside*,** as `Foo/` beside
  `repos/Foo/` is, and not a clone to move into a place that is taken.
- **The window moves only what its confirmation showed.** It sends that plan back, and a
  workspace whose clones at its root are others now, or that the confirmation did not list, is
  refused with nothing moved. The terminal shows nothing first, and moves what it finds.

Still open: the terminal reads the app's record of open chats, `reopen.json`, which is written
only for an approved plane, and no part of purlis sees a chat started outside the app.
