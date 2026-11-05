# Amended (2026-11-05; the person's ruling of that day): a chat in a branch folder leaves its clone's git to the app, and a clone's stopped rebase is resumed by the chat

A Claude Code chat started in a branch folder purlis cut has its clone's `.git` denied by path, so
every chat in a branch folder stands as one above it already does: the app commits, updates and
pushes for it, and read-only git works. `purlis worktree update --fetch-only` is how such a chat
refreshes what it knows of the remote. A worktree the person made elsewhere keeps its own git,
since the app commits for none of those.

For a sandboxed chat at or above a clone, a rebase stopped there goes on or back by the chat's own
git, inside its sandbox, and the app is asked only to put the branch where it belongs. Starting an update, a branch folder's `--continue` and `--abort`, and the
terminal do not change.

What the app notes of an update is kept in the project's state (`app/worktree-updates/`), never in
a git directory. A push's scan leaves out what the remote just said it has, and the app's git reads
no replacement objects.
