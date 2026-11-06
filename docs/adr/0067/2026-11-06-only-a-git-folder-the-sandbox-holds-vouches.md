# Amended 2026-11-06 (§5, class 5): only a git folder the sandbox holds vouches

**Decided by the person, 2026-11-06.** This narrows "The app's own work" in
[the amendment that tells what arrives](2026-11-05-a-file-that-can-run-later-is-told-when-it-appears.md).

Of the checks there, the ones that read a repository's branches, index, `worktrees` or `modules`
count only where the chat's sandbox holds that repository's git folder whole. Today that's the
clone of a branch folder a Claude Code chat works in
([a chat in a branch folder leaves its clone's git to the app](2026-11-05-a-chat-in-a-branch-folder-leaves-its-clones-git-to-the-app.md)).
A repository whose git folder is in the folders the chat may write, and isn't held, vouches for
nothing.

## What this costs

More is told, which is a false alarm at worst:

- an update of a clone in a chat's folders, such as a chat at the project's root pulling a
  workspace's clone, that brings one of these names;
- a branch folder or a submodule of such a clone;
- what a clone the app makes in a chat's folders holds. The clone's `.git` is still the app's.

A chat in a branch folder, and a repository outside the chat's folders, are as before.
