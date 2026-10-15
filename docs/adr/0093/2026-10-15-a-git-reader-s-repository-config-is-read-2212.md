# Amended 2026-10-15: a git reader's repository config is read (#2212)

Follows [the follow-ups of #1972 and #2203](2026-10-15-the-follow-ups-of-1972-2203.md).

- **A git reader in a repository whose config names a program counts as running one.** Where
  a line's only programs read or print, a git reader (`git log`, `git status`, `git diff` and
  the rest of `GIT_READERS`) still counts as running a program when its repository's own
  config names one it runs: `core.fsmonitor` (but `true`, git's own daemon, and `false`),
  `diff.external`, a diff driver's `command` or `textconv`, a filter's `clean`, `smudge` or
  `process`, `gpg.program` or `gpg.<format>.program`, `log.showSignature` set on, a
  `post-index-change` hook (git runs it when a reader writes the index it refreshed: in the
  hooks folder, in the folder `core.hooksPath` names, or a `hook.<name>` whose `event` it is),
  `extensions.partialClone`, and `remote.<name>.promisor` set on (`sandboxblock::drivers`). A
  `core.hooksPath` relative to a work tree whose top is not known counts as naming one.
- **What is read.** The repository is found as git finds it, up from the folder the command
  ran in to a `.git` folder or file. The folder is the one the command ran in, moved by each
  `cd` on the line before the reader, by a wrapper's own folder option and by `git -C`. Read:
  the git folder's `config` and `config.worktree`; for a linked work tree, its common git
  folder's (`commondir`) `config` and `hooks`; and each file those include (`include.path`,
  `includeIf.<condition>.path`, whatever the condition), four includes deep. Each file is read
  only when it is a regular file, and at most 256 KiB of it, so the hook stays cheap: a few
  small files, read only for a line that runs a git reader.
- **A repository named some other way counts as running a program**: a `cd` to a folder the
  shell expands (a variable, a substitution, a glob) or `cd -`, `--git-dir`, `--work-tree`, and
  `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `HOME` or `XDG_CONFIG_HOME` in the reader's
  environment.
- **Why the config, and not every refusal a git reader hands back.** Reading every git reader's
  refusals would undo #1972 for the lines that most often print refusal text: `git log` and
  `git diff` of a project whose tests and commits quote one, purlis's own among them. The
  config is a few small files, and it is where every driver a reader runs is defined.
