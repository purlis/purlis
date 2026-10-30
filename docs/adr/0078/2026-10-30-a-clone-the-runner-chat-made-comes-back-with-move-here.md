# Amended 2026-10-30: a clone the runner chat made comes back with Move here

**Built by #3811**, under spec #2041, after the amendment of 2026-10-29 (a workspace's chats move
together), whose item 4 left a clone made only on the runner there. The guards are the person's
ruling of 2026-10-27 (item 8 of that day's amendment), unchanged; the rest was **decided in
implementation, confirmed by the dispatcher 2026-10-30, following the ruling of 2026-10-27**
(D-3811-1 to D-3811-8 in the pull request).

1. **Session protocol 1.51: `clones_of`.** The window asks a runner which clones a project it
   keeps holds in a workspace, answered `clones` with each repository's folder name in the
   workspace's folder there, none reached through a link and none with a name a repository's
   folder may not have. The window's alone, relayed for it, as the other code commands are. A
   runner before 1.51 refuses it `unknown_command`, so the window asks only one of 1.51 or later.
2. **Move here carries each clone the runner holds and this machine has none of.** It is made
   here fresh: a repository with no hooks from a template, the runner's branches under
   `refs/remotes/<pool>/*` as a fetch keeps them, and its work tree checked out on the branch the
   runner's work tree is on, with its commits and its uncommitted work, by the same two-tree
   `read-tree -m -u` a clone that was here is taken over by (from the empty tree): no attribute
   read, no submodule reached, no hook run, and no path written through a link. What came is what
   both ends agree on after, so the next move to the runner takes its work tree over. No remote,
   config or hook of the runner's comes with it.
3. **Held to the same guards as the rest of Move here** (the ruling of 2026-10-27): only from a
   runner shown to be the person's own, as the move is asked and again at the switch; nothing
   written outside the clone; and **what came back in it that a tool here runs or reads, every
   file a save to the forge refuses and every link is named on the chat's tab**, as for a clone
   that was here, every file of it counting as new. Where what came cannot be listed, the whole
   clone is named.
4. **Never made over anything here.** Where a folder or file of that name is here and is not a
   repository, the move is refused in a sentence and it is not written into. A repository at a
   name a workspace keeps for its own (its memory, todos, session records and the like) is never
   a clone: a runner does not name it, and the window never makes one there. A clone that does
   not come leaves nothing here; the move stops there, as it does for a clone that was here.
5. **The row says which.** Move here's note says each repository the chat cloned on the runner
   comes back where the runner the window holds a link to is of 1.51 or later, and that one stays
   there otherwise, as before. The tab is drawn from a link already open, and never opens one.
