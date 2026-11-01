# Ruled 2026-11-01: where a chat runs resolves start, workspace, project, then this machine

**Ruled by the person on 2026-11-01, built by #4536.** It extends the amendments of 2026-10-20: a
workspace still names its pool per machine, and now a project and a start can too.

1. **The order.** A chat runs where its start chose, else where its workspace says, else where its
   project says, else on this machine. Most specific wins. `purlis_core::machine::Store::chat_place`
   is the one place it is written.
2. **Per machine.** `machine.json` keeps a project's pool (`projectChatsRunOn`) and the workspaces
   set to this machine (`chatsRunHere`), so a person's choice never changes where a teammate's chats
   run.
3. **A pool that's gone starts nothing.** A level naming a pool this machine doesn't keep with its
   runner refuses the start in words. It never runs the chat here in its place.

*Decided in implementation, to be confirmed:*

4. **A workspace has three settings**: Same as the project, This machine, or a pool. So This machine
   stays where the person put it after the project names a pool. A chat moved back here sets This
   machine. *(D-4536-1)*
5. **A chat at the project's root stays on this machine.** Code moves to a runner by a workspace's
   clones, so a project-root chat has nothing to carry. *(D-4536-2)*
6. **Removing a pool sends what named it to this machine.** A workspace that named it is set to This
   machine and never moves on to a pool its project names, since nobody chose that pool for it. The
   removal says so. *(D-4536-3)*
7. **A level purlis can't read stops the start** where the order would fall past it, until the
   setting is chosen again. *(D-4536-6)*
