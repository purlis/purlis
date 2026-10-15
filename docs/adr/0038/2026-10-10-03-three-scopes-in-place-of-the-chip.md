# Amendment, 2026-10-10: three scopes in place of the chip

The operator accepted B-15 of the sidebar grill on 2026-10-10 (#1671, #1679): the Chats view
switches between **This tab**, **Workspace** and **All**. The switch replaces the **all
workspaces** chip of the amendment above; there is one control, not two. It sits at the end of
the view's title line, a radio group drawn as one row of segments, one Tab stop with the arrows
between its three.

- **This tab** lists the chats of the tab in front, the set its chip counts and its menu lists
  (`tabChats.ts`), nested as the whole list nests them. A task in a tab of its own heads its
  tab's list.
- **Workspace**, the default, is the amendment above: the trees that started in the focused
  workspace.
- **All** lists every workspace's chats.

The pick is **kept for each project on this machine**, so it is there after a relaunch. It is
how one person looks at one project, like the folds. It was first kept in the window's web
storage, since `layout.json` had no per-project place for a view's state; since #1696 it is in
the project's entry of `layout.json` v2 (`projects[path].chats.scope`, B-11), beside Explorer's
folds (#1686), and the web storage key is moved there once. A row asked for outside the scope widens the scope
to the narrowest that lists it, and that is not kept, because the person did not pick it.

**In every scope, a chat the scope leaves out that needs the person is named at the top**: the
line under the filter says its name, how many more there are, and where they are ("outside this
tab", "in another workspace"), with a Go button to the one that has waited longest. A chat the
filter hides that needs the person is named the same way, first, and Go goes to it. The title
bar's needs-you queue still lists every chat.
