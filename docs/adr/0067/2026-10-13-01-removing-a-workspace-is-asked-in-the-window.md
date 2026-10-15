# Noted (2026-10-13, qw356): removing a workspace is asked in the window — to be confirmed

**To be confirmed by the operator.** Section 2 lists "creating and removing a workspace" among
the brokered writes. Creating one is brokered. Removing one never was: a chat held by the
sandbox was refused `purlis workspace remove` and told to ask the person, with nothing in the
window for the person to press. That was a dead end in the chat purlis's own Safe remove
opens. The operator's ruling of 2026-10-13 (workspace remove: the window asks Remove or Keep,
and removes on the person's click, for any chat) is built as follows.

1. **The chat's command asks, and removes nothing.** It hands the app a workspace's name and
   nothing else: no reason, no `--force`, no other project. The app answers with a sentence
   that says the person has been asked, or that this project has no such workspace.
2. **The app asks the person on the asking chat's tab and in the Inbox**: Remove… or Keep, in
   purlis's own words, with the name drawn as data and what the workspace holds counted by the
   app. The name is checked against the project's own list of workspaces when it is asked. A
   second ask by one chat about one workspace is the same question. A question about a
   workspace that is gone is dropped. Asks are held to the rate a chat's brokered writes are,
   and to three waiting per chat: a fourth is not asked, and none that waits is put away for
   it.
3. **Only the person's press in Delete workspace removes.** Remove… on the question removes
   nothing. It opens the window's own Delete workspace dialog, the one the sidebar opens: what
   goes, that there is no undo, the work nothing else has, and the core's guard behind its
   Delete. No command removes a workspace for a question; the ask adds one command to the
   window, Keep, and it is the window's alone (`session-protocol`'s `WINDOW_ONLY`). Decided in
   review, and the operator's to reverse: a removal is for good and it is a chat that raises
   the question, so it is not one press on a Notice, or on an Inbox row among Allows.
4. **The chat is told what the person decided on its next turn**, in purlis's fixed words, on
   the road a held connection's answer takes (the amendment of 2026-10-10, #1666). purlis
   types nothing into the chat for it.

So "removing a workspace" in section 2's list reads: **asked of the person through the app,
and never made for a chat.** No CLI word and no key of the project's format was added.
`purlis persona remove` and `purlis worktree remove` are still refused in a sandboxed chat;
the ask is shaped to carry each as one more kind.
