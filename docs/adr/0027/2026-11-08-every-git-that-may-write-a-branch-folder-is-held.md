# Amended 2026-11-08: every git that may write a branch folder is held to the one live writer

The amendment of 2026-10-30 (#3931) held a chat's plain git to a branch folder's claim where it
commits, updates or pushes. **Now the guard of a chat started without the sandbox judges every
git that may move a branch or HEAD, or write the index or the working tree, of a branch folder
the same way** (`purlis_core::branchgit::branch_write`):

- a commit: `git commit`, `merge`, `cherry-pick`, `revert`, `am`;
- a push: `git push`;
- an update: every other git, unless it only reads.

**What only reads is an allow-list, not a deny-list.** git's commands that never write (`log`,
`show`, `diff`, `status`, `blame`, `ls-files`, `rev-parse`, `cat-file`, `for-each-ref`, and
their kin), and the reading forms of those that may write (`branch` and `tag` listing,
`config --get` or a key alone, `stash list`, `worktree list`, `remote -v`, `symbolic-ref HEAD`,
`reflog` shown, a `fetch` that names no local ref to update, `apply --check`). Anything else
counts as a write, a subcommand the guard does not know included: an extension's `git-<name>`
program may do anything. A dry run, and a `--quit` or `--show-current-patch` of a sequence under
way, writes nothing. An `--abort` puts HEAD, the index and the tree back, so it is judged.

**A git is placed where it is pointed.** Its folder is the repository git will use (`-C`,
`--git-dir`, `GIT_DIR`, as git reads them), and the working tree it is told to write
(`--work-tree`, `GIT_WORK_TREE`, or the folder it runs in where only a git directory is given).
Where any of these is a place the line does not spell out, a git that may write is refused with
the shape that places it, as a line the guard cannot follow is.

The guard's tests keep a corpus of everyday read-only git that must pass unjudged; a reading
form missing from the allow-list is added there with its test.
