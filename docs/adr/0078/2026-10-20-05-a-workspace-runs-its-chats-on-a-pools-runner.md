# Amended 2026-10-20: a workspace runs its chats on a pool's runner

**Built by #2046** on §2, §4 and §7, after the ruling of 2026-10-20 that a workspace names its
pool per machine in `machine.json` (the amendment of that day, item 2). They are left as
accepted, and this note says how the window starts a workspace's chats there. *Decided in
implementation, to be confirmed* (D-2046-10 to D-2046-15, D-2046-20 to D-2046-22).

1. **The window routes each chat by its workspace.** A project's chats go through one router
   (`RoutedSessions`). A harness chat in a workspace whose chats run on a pool is started on that
   pool's runner by its names, over a link the desktop's host relays to it (`relay_to`). Every
   command about the chat after that goes to the same host: its views, what is typed, its size,
   its asks, its stop and its close. Every other chat runs on this machine. A workspace that
   names a pool this machine does not keep starts nothing, in words, and never starts here
   instead. The chat's pool is recorded before it starts, and kept only where the runner starts
   it under the number it was dealt. After a relaunch, a chat left running is asked of this
   machine's host and of the one pool its workspace runs its chats on, and no other.
   *(D-2046-12)*
2. **A shell runs on this machine**, wherever its workspace's chats run: its window composed it
   here. *(D-2046-13)*
3. **One project, one numbering.** This machine's host deals every chat's number, a runner
   chat's included, and the runner is given that number. So no two chats of a project share one,
   wherever each runs. *(D-2046-14)*
4. **This machine's host keeps the project's board** for every chat, those on runners included.
   It is held there before any chat starts on a runner, and the runner keeps none. *(D-2046-15)*
5. **The code goes as the chat starts, and comes back.** The project's own repository goes as the
   runner first holds the project. Each clone of the chat's workspace goes as the chat starts, on
   its branch with its uncommitted work (the amendment of this day on code moving as git
   bundles). The runner's branches come back into each clone's `refs/remotes/<pool>/*` as a
   runner chat ends, and when the window comes back into focus, at most once a minute. One
   fetch runs at a time, and chats that end together are fetched for once. Changes draws each
   one, its name drawn inertly, with the commits it has that this machine does not.
   *(D-2046-10, D-2046-11)*
6. **The setting is shown once a pool is kept.** The workspace's settings gain a *Runs on* group
   once this machine keeps a pool. It sets `chatsRunOn` for this machine alone, and only to a pool
   this machine keeps with its runner. A chat running when it changes stays where it runs.
7. **Each host is heard for its own chats alone.** Each pool's runner is heard through a hearing
   of its own beside this machine's host's, with the same board: its links, its `secret exec`s,
   its held asks and its programs are its own, since each host numbers its own lines. A hook
   line is heard only from the host its chat runs on: a pool's link never speaks for a chat of
   this machine or of another pool, and this machine's link never for a runner chat. A line that
   does is refused. *(D-2046-20)*
8. **Which host a chat runs on is the window's.** The setting is in this machine's store, which
   no chat writes; the router records each chat's pool; and no line or answer from a runner
   changes either. *(D-2046-21)*
9. **An older host is said so.** A move of code to a host before session protocol 1.34 is
   refused in purlis's words, by the link's version or by the host's `unknown_command`, and a
   refusal is said at once, stream or none. *(D-2046-22)*
