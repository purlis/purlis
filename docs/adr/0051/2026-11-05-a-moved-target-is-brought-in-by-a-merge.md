# Noted 2026-11-05 (#5338): a moved target is brought in by a merge

Decided in implementation, to be confirmed.

A plane in a request mode used to take its target only once its request had merged, so a request
that conflicted never recovered: every later save added to it, and the plane fell further behind.
The save and the app's look now bring a moved target in.

**A merge, not a rebase.** A rebase replays a line of saves commit by commit, and the `## Sessions`
line of `workspace.md` conflicts on every commit that wrote a record once it is settled to what
is true (`a_line_of_saves_each_writing_the_pointer_stops_a_rebase_on_each_and_a_merge_on_none`).
It also rewrites what the request carries. A merge stops once and rewrites nothing.

**Measured:** GitHub answers `mergeable: true`, `canBeRebased: false` for a request holding a merge
commit that settled a conflict (purlis-plane#2280, 2026-11-05), and `canBeRebased: true` for some
holding only clean merges (purlis#5304, #5263). So the merge reads `canBeRebased`, and a request
that answers false takes the next method the repository allows. A repository that allows only
rebase is still asked, and its refusal is what the request says.

**Made off to the side.** `git merge-tree --write-tree` touches no tree, index or ref, so a conflict
in what a person wrote changes nothing. That conflict stops the save and is recorded as a
`conflict` naming the files; the window's Resolve then leaves one real merge stopped on them.

**Generated files are rebuilt, never stitched,** from the merged tree: each place's
`sessions/index.md`, a workspace's `todos/MEMORY.md`, and the `## Sessions` line, whose count is
the index's lines and not the files. A union rule alone would keep them unconflicted and out of
order, so the rules serve only git run by hand or a plain rebase.

**In `pr-merge`, a look brings the target in only for a request the forge says cannot merge.** Each
new head starts the wait for its checks again, so a busy target could otherwise keep a clean
request from ever merging, and its merge brings the target in anyway. In `pr`, a look brings it
in whenever it moved; nothing there restarts a wait purlis depends on. A save brings it in either
way, since it pushes a new head regardless.

**Existing planes** take the new rules with their next whole save, which rewrites a block already
in `.gitattributes` and commits it; `reinit` adds a block to a plane with none. The rebuilds do
not depend on the rules, so nothing waits for them.
