# Noted (2026-10-14, qw382b): a save's conflict is settled by the window's git and a chat's edits — ruled by the operator, 2026-10-15

Decided under delegation, and ruled by the operator, 2026-10-15. Nothing in section 5 or in
the `rootupdate` guard changes: a sandboxed chat in the project's root still runs no git that
rewrites the tree from another commit, and `--abort` and `--quit` stay its only ways back. The
window's Resolve (ADR 0051, noted the same day) runs the replay, the staging and
`git rebase --continue` itself, under no sandbox, on the person's press. The chat it starts
edits the files git stopped in and nothing else. The window stages only files git stopped in
that the commit being replayed changes, and runs fixed commands. A conflict in a file section 5
denies a chat is not handed to one: the window says to settle it in a terminal.
