# Amended 2026-11-17: a fresh runner's data home is purlis, and a kept one stays charter

**Found on a live remote Docker pool**: Claude Code on the runner showed its project's folder as
`~/.local/share/charter/projects/<id>/workspaces/<name>`. The data home's folder is `purlis/` only
where rename-local made it (V93e, RN-5), else the newest old name, `charter/`, so that a first write
never starts a second folder beside the one an older build reads. Nothing runs rename-local on a
runner, so every runner kept `charter/`, the fresh ones too. *Decided in implementation, to be
confirmed* (D-7091-5):

1. **A runner's host begins its data home as `purlis/` where it has neither folder**
   (`datahome::begin_a_runners`, from `purlis serve --suspends-unlinked` and `purlis bridge`, the
   two ways a runner's `purlis` starts). No older build reads a fresh runner's home: its image and
   its `purlis` are one. A data home a variable names is the operator's, and nothing is made
   beside it.
2. **A runner whose home already holds `charter/` keeps it.** Its projects are git worktrees and
   clones whose `.git` files and `worktrees/*/gitdir` hold absolute paths under it, its chats'
   harnesses key their sessions by those paths (Claude Code's `~/.claude/projects/<path>`), and a
   running chat's folder is under it. Renaming it under them would break each, so moving a kept
   runner's home is a migration of its own (#7114), not part of this change.
