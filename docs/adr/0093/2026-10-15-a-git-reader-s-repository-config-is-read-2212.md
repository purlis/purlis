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
  relative `core.hooksPath` is read against the work tree's top, the folder its `.git` was
  found in.
- **What is read.** The repository is found as git finds it, up from the folder the command
  ran in to a `.git` folder or file. The folder is the one the command ran in, moved by each
  `cd` or `pushd` on the line before the reader (past its options), by a wrapper's own folder
  option and by `git -C`. Read:
  the git folder's `config` and `config.worktree`; for a linked work tree, its common git
  folder's (`commondir`) `config` and `hooks`; and each file those include (`include.path`,
  `includeIf.<condition>.path`, whatever the condition), ten includes deep as git itself. Each
  file is read only when it is a regular file of at most 256 KiB, and at most 32 files are
  read for one answer, only for a line that runs a git reader and only where the result's
  answer is not already decided (a failed command, a container client).
- **What cannot be read counts as naming a program**, since the answer decides whether a
  refusal is raised: a walk to the repository that stops on an error, a `.git` file that names
  no folder, a config or include that is there but cannot be read, is not a regular file or is
  past the bound, an include past git's depth, a reading past its 32 files, and a `~/` path
  with no home known. Only a file that is not there and no `.git` up to the root name none.
  `gitconfig::git_dir_at`, which the plane-root guards read, answers as before.
- **A repository named some other way counts as running a program**: a `cd` to a folder the
  shell expands (a variable, a substitution, a glob) or `cd -`, `--git-dir`, `--work-tree`, and
  `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `HOME` or `XDG_CONFIG_HOME` in the reader's
  environment.
- **Why the config, and not every refusal a git reader hands back.** Reading every git reader's
  refusals would undo #1972 for the lines that most often print refusal text: `git log` and
  `git diff` of a project whose tests and commits quote one, purlis's own among them. The
  config is a few small files, and it is where every driver a reader runs is defined.
