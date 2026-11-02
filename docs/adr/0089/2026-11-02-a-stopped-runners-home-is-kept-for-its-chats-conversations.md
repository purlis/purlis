# Amended 2026-11-02: a stopped runner's home is kept for its chats' conversations

*Ruled by the person, 2026-11-02 (#4633). The details below it were decided in implementation,
delegated, to be confirmed.* This settles the open part of the amendment of the same day, *a
draining runner found stopped is held with its home*: its item 5 (an update on a runner already
stopped) and the end of its item 6 (one not yet held whose container was removed outside purlis,
#4168). It changes item 5's Replace too.

**The ruling.** Before an update, a Replace or an outside prune removes a stopped runner's home,
purlis checks whether that home holds any chat's conversation. If it does, the home is kept, and a
Notice offers to bring the conversations back or discard them. Discard asks twice. A home with no
conversation is removed as it is today. The same rule covers a draining runner not yet held whose
container was removed outside purlis.

1. **What is checked.** A draining runner that a look would remove with no chat on it, where it is
   stopped: one that had stopped before its drain began (after an update or a Replace), and one
   not yet held whose container the engine that made it no longer lists, its home still there and
   its own (#4167). A runner that runs and answers that it has no chat goes as before: its chats
   ended while it ran, so the person ended them.
2. **How it is checked.** A short-lived container of the pool's runner's image, as the sign-in
   carry runs (#4011), with no network and no capability, mounts the old home read-only and lists
   every plain file in the folders a harness keeps its conversations in: Claude Code's `projects`
   (its transcripts, and its subagents' and tool results beside them) and Codex's `sessions` and
   `archived_sessions`, in a made profile's folder and in each harness's default folder, and
   opencode's storage. The runner is never started to be asked. **It fails closed**: a program
   the image lacks, a folder it cannot read, a Docker that does not answer whether the container
   is there, or any read that does not finish says nothing of the home, so the runner stays as it
   is, neither held nor removed, and the next look reads it again. It is never taken for an
   empty home.
3. **A home that holds one holds the runner** (`held`, as a stop during its drain does). The
   Notice is in Devices, one for each held runner, whatever held it.
4. **Bring back conversations** puts each of those files at the same place in the pool's runner's
   home, where its harness resumes a conversation by its id. Nothing is written through a link or
   over a file: one already there counts as brought back only where it is the same. Only once none
   is left behind is the old runner removed, with the rest of its home, as Remove runner removed
   it, its sign-ins carried; the Notice says so before the press. One left behind keeps it held,
   and the window says how many came back. A home that holds no conversation is never removed by
   it, which a runner held for a stop during its drain may be: that one goes only by Discard. A
   held runner a link started again is stopped first, so nothing changes under the copy (no chat
   starts on a draining runner). opencode keeps a conversation over many files that only its own
   import reads, so its conversations are seen and kept but never brought back. Picking a chat up
   from there in the window is still #4632's.
5. **Bring back is refused**, and the runner stays held, where a chat ran on either runner without
   the sandbox, or the two run as different users: the same rules as the sign-in carry.
6. **Discard asks twice**, then removes the runner and its home as Remove runner did. It replaces
   Remove runner on the runner's row.
7. **Replace holds a runner already stopped only as an update does**: by what its home holds. The
   earlier amendment's item 5 held it on the press alone; the ruling's "a home with no conversation
   is removed" now covers it too.
