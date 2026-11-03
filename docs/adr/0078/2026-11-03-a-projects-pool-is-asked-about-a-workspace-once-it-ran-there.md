# Amended 2026-11-03: a project's pool is asked about a workspace once it ran there

**Built by #4591**, on the amendment of 2026-11-01, *where a chat runs resolves start, workspace,
project, then this machine* (#4536), and the amendment of 2026-11-02, *a chat is found on the pool
it ran on* (#4590). The person's rulings in both stand as they are. Everything here is *decided in
implementation, to be confirmed* (D-4591-1 to D-4591-6).

1. **Fetch-back and the away list ask about a follower once it has run there.** A workspace that
   follows the project's pool is asked about on that pool once a chat of it is kept there, or its
   branches are still owed from there (`pools::workspaces_to_fetch`). Before that, nothing of it is
   there to come back. A workspace that names a pool of its own is still asked about as its
   setting says, because a chat of it from before those records may have left work there.
   *(D-4591-1)*
2. **A put-back still closes by the settings.** Point 4 of the amendment of 2026-11-02 holds as it
   stands. On a pool the settings run any workspace on, the project's pool included while a
   workspace follows it, a put-back closes every chat that nothing took back. That holds whether
   or not a follower has run there, so a chat found with no record and not taken back is never
   left running (`pools::workspaces_on_pools`). *(D-4591-2)*
3. **A project is marked as not held on its pool only while a workspace follows that pool.** If
   the project's workspaces can't be listed, the pool still counts. Each change of Runs on
   rechecks the marks. *(D-4591-3)*
4. **A pool named for a workspace that has no folder becomes This machine.** When the window opens
   a project, a workspace in `chatsRunOn` that has no folder under `workspaces/` moves to
   `chatsRunHere`. A folder later made or brought back under that name then runs on this machine
   and never takes up a pool unasked. A This machine setting is never dropped. Nothing moves while
   any of the workspaces can't be read or a rename is under way. A chat's record and the branches
   still owed under that name stay, and a put-back settles them. The move is permanent. A folder
   that was gone only for a moment, such as during a checkout, comes back on This machine, and
   the person sets its pool again. That is the safe side of the choice. *(D-4591-4)*
5. **A move that can't put Runs on back names the setting the workspace had**: Same as the project,
   This machine or a pool, by the labels its settings use. *(D-4591-5)*
6. **Start chat says a workspace's first chat on a pool.** When a workspace has never run a chat on
   the pool a start would use, the Runs on row says before anything is sent that its code is
   copied to that pool's runner and that the chat's work comes back here. That pool may come from
   the settings or be picked for this start. The workspace has run there when a chat of it is kept
   there, its branches are still owed from there, or a clone of it holds branches that came back
   from there (`pools::pools_run_on`). A workspace with no clone sends nothing, so the row says
   nothing. This is the word a workspace renamed outside purlis gets
   before its new name follows the project's pool. *(D-4591-6)*
