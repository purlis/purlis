# Amended (2026-10-13, #1476): follow-ups of the brokered commit, decided in implementation

Decided under delegation and flagged for the operator. Each narrows what the amendment of
2026-10-08 allowed, or keeps it, except the first, which amends its "Not covered" line.

1. **A chat standing above branch folders commits in the one below it that its paths name.**
   A chat started in a workspace (case B), in the folder that holds a repo's branch folders, or
   at the project's top, names each path from where it stands
   (`.worktrees/<repo>/<piece>/<path>`, or the branch folder itself for every change in it).
   The ask still names no folder: the app reads the folder from the paths, every path must be
   in the same branch folder, that folder must lie **below** the folder the app recorded the
   chat as standing in, and no part of the way to it may be a link. Everything decision 2 to 8
   say then holds for that folder, as for a chat standing in it. `--all` names no path, so it
   is refused there with a sentence. The chat could already write every file in that folder;
   the commit writes the same git records a chat standing in it may ask for.
2. **A chat is held to a number of commits in a while**: 30 in ten minutes, counted per chat
   and project for the app's life (`gitbroker::commit::COMMITS_A_WHILE`).
3. **A message carrying one of purlis's own provenance trailers is refused**
   (`Purlis-Chat`, `Purlis-Persona`, `Purlis-Change`, under every name they have had, in any
   case). purlis writes those for the commit it makes, and one the chat wrote would claim what
   purlis did not check. A chat's own commit in a clone is unchanged: ADR 0074 says its hook
   only appends and never refuses, so the same rule there is left to the operator.
4. **A commit purlis's scan refuses raises a needs-you item**, with the masked line a chat's
   own `pre-commit` sends (SQ-16), and an event in the log.
5. **Kept as decided**: the repository's own hooks do not run (decision 3), a file marked for
   a content filter is refused (decision 4), and no git starts inside a submodule
   (decision 8). Each would run a program, or read a config, that this commit does not.

Decision 8 is narrowed three ways:

- **"Below a workspace" is below one of a project's own workspaces**: the folder above
  `workspaces` holds a project manifest, which no chat writes. A folder that only happens to be
  named `workspaces` is not one.
- **A checkout below a workspace that keeps its git directory elsewhere**
  (`git clone --separate-git-dir`) is still not followed, and is refused with a sentence of its
  own (`worktree::link::ELSEWHERE`) where the link names a whole git directory. Nothing in such
  a directory names the checkout back, so purlis cannot tell the person's layout from a link a
  chat wrote to a git directory it prepared. A way to tell them apart is left to the operator.
- **The runners that answer a chat's own git questions are held to the check** (the
  project-root guards, the hook scan), with the same sentences. Past it they still find the
  repository as the session's own git does.

A caller that read a refused call as "not a repository" now says the check's sentence
(`worktree::not_read`).
