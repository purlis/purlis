# Amended 2026-10-20: a workspace runs its chats on a pool's runner

**Built by #2046** on §2, §4 and §7, after the ruling of 2026-10-20 that a workspace names its
pool per machine in `machine.json` (the amendment of that day, item 2). They are left as
accepted, and this note says how the window starts a workspace's chats there. *Decided in
implementation, to be confirmed* (D-2046-10 to D-2046-15).

1. **The window routes each chat by its workspace.** A project's chats go through one router
   (`RoutedSessions`). A harness chat in a workspace whose chats run on a pool is started on that
   pool's runner by its names, over a link the desktop's host relays to it (`relay_to`). Every
   command about the chat after that goes to the same host: its views, what is typed, its size,
   its asks, its stop and its close. Every other chat runs on this machine. A workspace that
   names a pool this machine does not keep starts nothing, in words, and never starts here
   instead. *(D-2046-12)*
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
   runner chat ends, and when the window comes back into focus, at most once a minute. Changes
   draws each one with the commits it has that this machine does not. *(D-2046-10, D-2046-11)*
6. **The setting is shown once a pool is kept.** The workspace's settings gain a *Runs on* group
   once this machine keeps a pool. It sets `chatsRunOn` for this machine alone, and only to a pool
   this machine keeps with its runner. A chat running when it changes stays where it runs.
