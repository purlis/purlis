# Amended (2026-10-30, #3895, #3890; extends "a branch folder is kept current by the app"): a branch folder pushes over its own update's rewrite with later commits on top, and is cut from and updated onto the remote base where the local base diverged

Both were decided in implementation. Neither adds a command word.

1. **A push goes over the remote's copy only where purlis's own update rewrote the branch
   (#3895).** Point 2 of the 2026-10-14 amendment gave `--force-with-lease` only while the
   branch was exactly where the last update left it, leased to what the remote had when the
   push fetched it. So one commit added after an update made the branch impossible to publish
   without rebuilding the pull request on a new branch. Now `purlis worktree update` notes, in
   the folder's own git directory, which no chat writes, what the remote's copy of the branch
   was at when it rewrote the branch. It notes this only where the folder had every commit of
   that copy. A second update before the first was pushed carries the first one's note
   forward, where the remote's copy is still the one it noted and the branch still holds what
   it made. Where the remote's copy holds commits the folder never had, the update says so
   and notes nothing. `purlis worktree push` goes over the remote's copy only where that note
   names the branch, the commit the update left the branch at is in what is pushed (commits
   added since are fine), and the remote's copy is still at the noted commit. It then pushes
   with git's own lease on that commit, so a remote moved in between is refused by the remote
   itself. Anything else that does not fast-forward is refused: a remote someone else moved, a
   branch rewritten any other way, a note purlis cannot read or one an earlier purlis wrote.
   The refusal names what to do. The note is spent by the push that uses it, and is trusted
   only in the clone's own `.git/worktrees/<piece>` record of that very folder.

   A plain push does all of this. `--force-with-lease` is still accepted and asks for nothing
   more, so a chat never needs a force push approved: the push is purlis's own operation on
   the folder's own branch, never the branch the remote names its default.

2. **A base that has diverged from its remote is used as the remote has it (#3890).** Where
   the clone's base branch and its remote-tracking branch have each moved on, a cut and an
   update start from the remote-tracking branch, as they already did for a base that was
   behind and checked out. The base branch is never moved, and stays the base recorded for
   the piece. A base only ahead of its remote is used as the clone has it, since its own
   commits are the base's.
