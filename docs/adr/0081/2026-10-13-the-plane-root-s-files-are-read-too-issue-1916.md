# Amended 2026-10-13: the plane root's files are read too (issue 1916)

The file manager read a workspace's repo or branch and nothing else, and the core refused a
folder that git answers for only because the project around it is a repository: listing it would
list the project's own files (`files.rs`). The operator ruled on 2026-10-13 that the plane root's
tab shows the whole project's files (rulings G3, F1 and F2). So the project's own folder is a
place the file manager reads, and the rule above stands for every other place: a folder inside
the project that is no repository of its own is still no repo of a workspace.

- **Read only, as every place is** (§2): the plane root's tree creates, renames, moves and
  deletes nothing.
- **What it leaves out** is in ADR 0038's amendment of the same day and in `files/kept.rs`: what
  purlis keeps to itself, and each workspace's clone, which is one row that leads to its
  workspace.
- **Your editor is not started from it** in this change (§3): a file of the plane root is read
  in the light editor, and the hand-off, which names a branch, is a follow-up.
