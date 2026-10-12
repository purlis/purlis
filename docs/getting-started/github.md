# Getting started with GitHub

This page takes you from nothing installed to a chat working in one of your GitHub repos. The
same steps for GitLab are in [Getting started with GitLab](gitlab.md).

## What you need

- A Mac with Apple Silicon, or a Linux machine on x86_64. There is no Windows build yet.
- At least one harness, the coding agent a chat runs: Claude Code (`claude`), Codex or
  opencode. Each one signs in with its own login, the first time a chat starts it. With none
  installed, purlis lists each one's official installer once your repo is open, and
  **Install** runs it in a shell tab.
- GitHub's own command-line tool, [`gh`](https://cli.github.com/), signed in with
  `gh auth login`. purlis lists your repos, clones them and opens pull requests through `gh`,
  as you: it keeps no GitHub token of its own.

## Install

Download the newest release from the
[releases page](https://github.com/purlis/purlis/releases/latest):

- **macOS:** `purlis-macos-arm64.dmg`. Open it and drag **purlis** to Applications. The build
  is not notarized, so macOS refuses the first launch from a download. Run this once, before
  the first launch:

  ```sh
  xattr -dr com.apple.quarantine /Applications/purlis.app
  ```

  To use the `purlis` command in a terminal, run **Install `purlis` command in PATH** from the
  command palette.
- **Linux:** `purlis-linux-x86_64.deb` (`sudo apt install ./purlis-linux-x86_64.deb`, which also
  puts `purlis` on your `PATH`), or the AppImage, `purlis-linux-x86_64-appimage.AppImage`.

After that, purlis updates itself: it checks the release's signature before it installs
anything, and it asks before it installs. A `.deb` install is updated by installing the next
`.deb`. [How purlis updates itself](../updating.md) has the details.

## The first run

The first window is the welcome. It offers three doors: **Create a project**, **Get a project
from your forge** and **Open a folder on this machine…**, with **Sign in to GitHub or GitLab**
under them and your recent projects below. It looks the same whatever your machine
remembers.

Press **Create a project**, then pick a repo folder on your machine with **Open a repo…**, or
type its path. purlis then makes a project for you. A project is a git repository where purlis keeps your
workspaces, personas, memory and settings, and this first one lives on this machine only. Your
repo is cloned into a workspace named after it, and the first chat starts in that clone.
Nothing is written into the repo you picked.

Under **On this machine**, the page shows which harnesses it found and whether each is signed
in.

**Sign in to GitHub or GitLab** shows whether `gh` and `glab` are installed and signed in. A
GitHub user needs only `gh`. If `gh` is installed but not signed in, **Sign in to GitHub**
opens a shell tab running `gh auth login`. Signing in from purlis itself, with no `gh`, comes
in a later update, and so does **Get a project from your forge**: until then, clone a
teammate's project with git and open it with **Open a folder on this machine…**.

## Working with your GitHub repos

The project purlis makes on the first run tracks the forge your repo's `origin` is on, and its
owner: a repo cloned from `github.com/acme/widget` gives a project that tracks GitHub, owner
`acme`. If the repo has no remote, or its remote is not on github.com or gitlab.com, the first
run asks which forge before it makes the project. To list and clone the repos under your account
or organisation:

1. Open **Project settings…** and, under **Forges**, check the first forge's **owner**: your
   GitHub user or organisation. Leave **host** empty for github.com, or set it to your GitHub
   Enterprise Server's host, such as `github.example.com`.
2. Open **New workspace…**. Its repo picker lists the repos your own `gh` login reaches under
   that owner, asked when the dialog opens; **Refresh** asks again. Pick the ones this piece of
   work needs. A workspace with no repos is fine too.
3. purlis clones each one over HTTPS from its default branch, and sets the clone up to fetch
   and push with `gh auth git-credential`, so git uses your `gh` login.
4. Start a chat in the workspace and choose its harness.

If the picker cannot ask GitHub, it says why in `gh`'s words, for example that `gh` is not
signed in for that host, and tells you the command to run.

From a terminal, `purlis init --forge github --owner <owner>` makes a project in the current
directory that tracks that owner. Add `--host <host>` for GitHub Enterprise Server. With
`--adopt <repo>`, `purlis init` reads the forge and owner from that repo's `origin` instead, and
without either it asks you to name the forge.

## Saving your project to GitHub

A project is saved with git, as far as its **mode** says, and each mode includes the ones
before it:

| Mode | A save… |
| --- | --- |
| `off` | is never committed |
| `commit` | commits on this machine |
| `push` | also pushes to the project's branch |
| `pr` | pushes to a save branch and opens or updates one pull request |
| `pr-merge` | also turns on auto-merge for that pull request, or, where the repo has no auto-merge (a private repo on GitHub Free), merges it once its checks pass |

Until a mode is set, the **Saving** view asks once, before anything is pushed. The project from
the first run has no remote, so only `off` and `commit` have anywhere to go until you push it to
a GitHub repository of its own. The keys are in [the project format](../plane-format.md#chartertoml).

## The five words

purlis has five concepts, and every other word belongs to one of them
([ADR 0072](../adr/0072-charter-has-five-concepts-and-every-other-word-belongs-to-one-of-them.md)):

- **Project:** the git repository that holds your workspaces, personas, memory and settings. A
  code repo is never a project.
- **Workspace:** a named piece of work, with its own `workspace.md`, memory, todos and repos.
- **Chat:** one conversation with an agent, in a tab.
- **Persona:** a role a chat can take, with its own instructions, memory and vault.
- **Memory:** what purlis keeps so that the next chat knows what earlier ones learned.
