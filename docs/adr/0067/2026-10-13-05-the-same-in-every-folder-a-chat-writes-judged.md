# Noted (2026-10-13, qw349): the same in every folder a chat writes, judged by what differs

The later-code names of section 5 are denied at any depth of every folder a chat writes, so
the half-done checkout noted above is met more often in a workspace's clone or a branch folder
than in the root: any repository that tracks an editor's tasks, a hook manager's folder or an
agent's settings. Measured on git 2.50 with a path the kernel refuses writing, in a clone:
`pull`, `merge`, `rebase` and `reset --hard` wrote the other files and failed with the commit
unmoved; `checkout` and `switch` did the same and exited 0 at the new commit, the file left
behind as a local change; `clone` wrote what it could, left the index empty and said the
checkout failed.

Nothing in section 5 changes: those files run code later, and a chat does not write them.
Decided by the implementing lanes, to be confirmed by the operator:

1. **Outside the root the guard is precise** (`cloneupdate`, `docs/hooks.md`). It asks git
   which paths differ between the folder's commit and the commit a command moves it to, and
   refuses only a move that would write a denied path, naming it. Where the commit cannot be
   named before the command runs (a `pull`, any command that may write before it on the
   line, a form whose commits are not on the line) it refuses only where the repository tracks a denied path, as
   the root's guard does for its folder, and says how to make the move one it can check.
2. **The root's guard stays as it was**, refusing every such update there. The project's
   manifest is denied there too, by where it is and not by its name alone; the app's own save
   commits whatever is in the root; and a merge's directory-rename detection can write a path
   neither commit's difference names. One miss costs more there than a refusal does.
3. **A hook asking git about a folder a chat chose asks only what cannot start a program.**
   Through the one hardened runner, with no global or system configuration, and by plumbing
   that compares commits; never a file's content through a filter.
4. **Where it cannot ask, it does not allow.** A repository it could not read, a commit git
   did not name, and a folder that was never checked out are not taken for a folder with
   nothing to stop at. The way back to the commit a folder is at stays open.
5. **A net under the guard.** After a git command that said it could not write a later-code
   file, the chat is told what the folder now is and the way back, in that turn.

Left for a ruling: whether a clone a chat makes for its own work should have its later-code
files written by the app on the chat's behalf, as `purlis clone` already makes a whole clone
of a repository in the project's inventory. Until then such a clone lacks those files, and a
commit made from it without care records them as deleted.
