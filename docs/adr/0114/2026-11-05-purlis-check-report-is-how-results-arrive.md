# Amended 2026-11-05: `purlis check report` is how results arrive

No check is run by purlis yet (ED-8 is Later), so results need a way in for a script, an agent's
hook or CI. The operator approved the command on 2026-11-05 (#5407), and ED-8 and the Check reporter
(#5410) will feed the same path:

```
purlis check report <branch> <file.sarif> [--repo <repo>] [--workspace <workspace>]
```

- **`<branch>` is one a folder of the workspace has checked out**, a branch folder or a clone. A
  name two repos have is refused, asking for `--repo`.
- **Each tool's results replace that tool's alone**, and each set records the commit the branch was
  at when it was reported. Once the branch's head moves, the set is drawn out of date. It records the
  commit only, so a check run over uncommitted changes is not told apart from one run without them.
- **A file that is not SARIF 2.1.0 is refused**, exit 1, saying what is wrong and what to do, and
  what was recorded before stays.
- **In a sandboxed chat the app makes the write** (ADR 0067 §2), since a chat's sandbox does not
  write the workspace's `pieces/`. The file can be far longer than a line of the chat's connection,
  so the app reads it. It reads only a file inside the named branch's folder, in the chat's own
  workspace, following no link, so a chat cannot have it read a file its sandbox denies it.

The store is in `docs/plane-format.md`.
