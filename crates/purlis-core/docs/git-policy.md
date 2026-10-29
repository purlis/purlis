# The one-credential rule

Every git operation purlis performs — from any persona, any sub-agent, any repo clone —
authenticates with **that repo's own forge's CLI token, over HTTPS**: `glab` for GitLab,
`gh` for GitHub. Never an SSH key, never commit or tag signing. The one exception is a clone
the person starts and watches from the window ([The person's own clone](#the-persons-own-clone)).

```bash
purlis git-policy            # check every clone against the rule
purlis git-policy --apply    # write it into each repo's local git config
```

`--apply` writes a credential helper, `commit.gpgsign = false`, and SSH→HTTPS URL rewrites
into a repo's *local* git config, so even a repo whose remote is an SSH URL still
transports over HTTPS. `purlis clone` applies it automatically to everything it clones.

A directory purlis cannot read — `workspaces/` itself, one workspace, or a clone's `.git` —
is named rather than skipped. `purlis git-policy` warns about each one with what clears it,
and does not count a repo it never reached. `purlis doctor`'s `git auth` row runs the same
check, read-only: it reports which repos drifted and names `purlis git-policy --apply`, and
never applies anything itself.

## Why it is a rule and not a preference

An **SSH key passphrase prompt or a GPG signer prompt hangs an autonomous agent** mid-run.
There is no human at the keyboard to answer it, so the run does not fail — it sits there
until something times out, which is a worse failure than an error because nothing reports
it. One credential, held by the forge's own CLI, over HTTPS, is the only shape that can
never block on a question nobody is there to answer.

It also collapses credential management to one place. The forge CLI already handles
storage, refresh and revocation; purlis borrows that rather than inventing a second
system that would need its own rotation story.

## The guard

purlis's `PreToolUse` guard — the Bash guard every chat the app starts runs — **denies** a command that would route around
the rule:

- a raw SSH GitLab/GitHub URL handed to git
- `GIT_SSH_COMMAND=`
- `-S` / `--gpg-sign`
- `ssh -T git@github.com`

**If you hit one of these denials, that is the rule working, not a bug.** The message names
the fix, which is usually nothing at all — `purlis git-policy --apply` has already
configured the repo correctly. Check the credential with `glab auth status` or `gh auth
status`, never `ssh -T`.

**And when it is not the rule working** — a repo that genuinely needs something this guard
refuses — see [hooks.md](hooks.md) → *When a guard is wrong*. Short version: nothing in
`charter.toml` or the environment lifts a denial, deliberately, and you run the command in
your own terminal. If the guard is wrong about you *every time*, that is purlis holding a
policy your organisation does not, and it belongs in an issue rather than a local switch.

The same guard covers the vault files under `.charter/vaults/`: it refuses reading one
directly, so an accidental `cat` cannot put a secret in the transcript. That is an
accidental road, and the kind a name-based guard can close — a command you chose to run is
not one of them. See [secrets.md](secrets.md) and [hooks.md](hooks.md).

## Submodules are outside the rule, and purlis says so rather than reaching past it

`purlis clone` clones the repo you named and **does not initialise its submodules**.
`purlis clone`, `purlis sync` and `purlis status` each say when a clone has submodules
with nothing checked out, name them, and print the one command that fixes it:

```bash
git -C workspaces/<ws>/<repo> submodule update --init --recursive
```

It is yours to run, and that is a decision rather than an omission.

**A submodule URL is not a URL purlis built.** It comes out of `.gitmodules`, a file
inside the repo that was just cloned, and it can name any host, recursively.
`purlis clone` already refuses to hand `git clone` a URL purlis did not build from the
forge's host and the repo's path — `ext::sh -c '…'` is a transport that runs a command —
and fetching whatever
`.gitmodules` names would put that string back one layer down, where the allowlist above
it cannot see it.

**And the policy on this page does not reach a submodule fetch anyway.** Everything
`--apply` writes goes into a repo's *local* config, and **`git clone` does not read the
local config of the repository it is standing in** — system, global and `-c` only. A
submodule fetch *is* a nested `git clone`. Measured on git 2.50.1, against a superproject
whose submodule cannot be fetched at all without the config under test:

| where `protocol.file.allow` was set | `git submodule update --init` |
| --- | --- |
| the superproject's **local** config | fails — never read |
| `-c` on the command line | succeeds |
| **global** config | succeeds |
| **local** `submodule.<name>.url` override | succeeds |

The last row is the asymmetry: the *parent* (`git submodule update`) resolves the URL from
local config, so a `submodule.<name>.url` override works; the *child* (`git clone`)
consumes `credential.helper` and `url.<https>.insteadOf`, and its config search skips the
local file that holds them. So a submodule fetch runs **without** purlis's credential
helper and **without** its SSH→HTTPS rewrite, whoever starts it. A purlis that
initialised submodules for you would be fetching outside its own credential policy,
quietly, with your token in reach.

Two practical consequences if your repos keep tooling in a submodule:

- A submodule whose `.gitmodules` URL is SSH will go over SSH — the rewrite is not there.
  `git config submodule.<name>.url https://…` in that clone is read by the parent and
  fixes it for good.
- The guard is **not** what stops you. It reads the command line, never `.gitmodules`, so
  `git submodule update --init` is not denied by anything. What it *does* deny is typing an
  SSH URL while you configure the override — write the HTTPS form instead.

## Every git purlis runs goes through one runner

A repo's own config can name programs, and git runs them: a hook, an fsmonitor, a pager, an
alias. A chat can write the repo it works in, and purlis runs git there from outside
any sandbox. So the rule ADR 0084 states for the review engine holds for every git call purlis
makes (#810): **git run by purlis on a repo an agent can write must not execute programs named
by that repo's config.** It is held in one place, `worktree::git` (the runner), and not at each
call site.

### What the runner holds on every call

- **No hook and no fsmonitor.** `core.hooksPath` and `core.fsmonitor` are turned off with `-c`
  on the command line, which beats every config file.
- **No program the repo's own config names** (#1805). Before a call that can reach one, the
  runner lists the repo's config and turns off, with `-c`, each of these that the repo sets
  (any scope but your global and system files). The listing is kept for the repository, asked
  the same way, while nothing it was read from has changed: each file it says it read, each
  config file git would read had it been there (the repo's, the worktree's, your global ones),
  and each `.git` on the way up to the repo are checked again before every call, by device,
  inode, size, modification time and change time, which no writer can set back. A listing with
  an include in it, or one asked on a pinned thread, is taken again for every call:

  | Key | Starts | What purlis does |
  | --- | --- | --- |
  | `filter.<name>.clean`, `.smudge`, `.process` | a filter, on checkout, status, add | turned off, and the filter is no longer `required` |
  | `diff.<name>.textconv` | a textconv, on diff, blame | turned off |
  | `diff.<name>.command`, `diff.external` | an external diff | turned off; patch verbs are also given `--no-ext-diff --no-textconv` |
  | `merge.<name>.driver` | a merge driver | turned off: a merge that needs it stops on a conflict |
  | `gpg.program`, `gpg.<format>.program`, `gpg.ssh.defaultKeyCommand` | a signer, or a signature check | turned off: a commit or tag that would sign there is refused before git runs, and the refusal says to set the signer in your global git config (with `includeIf` for a per-folder signer); `log` and `show` never check signatures (`log.showSignature=false`) |
  | `core.editor`, `sequence.editor` | an editor | turned off: a command that needs one fails |
  | `trailer.<name>.cmd`, `.command` | a trailer's command | turned off |
  | `core.alternateRefsCommand` | a listing of an alternate's refs | turned off |
  | `hook.<name>.command`, `hook.<name>.event` | a hook named in config (git 2.54 and later), which `core.hooksPath` does not reach | that hook is turned off by its name (`hook.<name>.enabled=false`); a verb that writes a ref (`update-ref`, and `branch` or `symbolic-ref` when they write) is listed for it too |
  | `remote.<name>.uploadpack`, `.receivepack` | a program at the other end of a remote | git keeps the first one it reads, so `-c` cannot turn it off: a fetch, pull, push or `ls-remote` there, or a `remote` that asks the remote, is refused |
  | `remote.<name>.vcs` | the remote helper git starts for that remote | an empty one names a helper too: a call that reaches the remote is refused, as above |
  | `gc.recentObjectsHook` | commands a gc runs (git 2.42 and later) | a list that no `-c` empties: the gc and maintenance a commit, merge or fetch starts on its own are turned off (`gc.auto=0`, `maintenance.auto=false`), and a `gc`, `repack`, `prune`, `maintenance` or cruft `pack-objects` is refused |

  **Git LFS keeps working.** The three values `git lfs install` writes are left alone, and
  `git-lfs` is found on the runner's fixed `PATH`. If the repo's config also names a program
  `git-lfs` itself would start (its transfer agents or extensions, a credential helper, an ssh
  or askpass program), the LFS filter is turned off there too, yours included.

  Keys that cannot run, or that the runner already holds another way, are not turned off:
  `core.pager` and `pager.<verb>` (git's output is never a terminal), `alias.<verb>` (never
  taken under a verb of git's own), `credential.helper`, `core.askPass` and `core.sshCommand`
  (only network calls ask for a credential or a transport, and those hold the one-credential
  rule above), `core.gitProxy` (only `git://`, which every network call refuses),
  `uploadpack.packObjectsHook` (git reads it only from your own config), and the keys of verbs
  purlis never runs (merge and diff tools, `send-email` and its commands, a browser, a GUI tool,
  `interactive.diffFilter` for `add -p`, and `submodule.<name>.update` for `submodule update`,
  which only the network entry point could run). Checked against `git help --config` of git
  2.50 (#1853). The planted test
  plants the credential helper, the askpass program, the ssh command and the diff and merge
  tools too, so a call that one day reached one is caught.
- **No git of its own inside a submodule**, and no lazy fetch of a missing object, which would
  start the program a promisor remote's config names.
- **A bare repository only where it is named** (`safe.bareRepository=explicit`).
- **A constructed environment**: the child gets `HOME`, a fixed `PATH`, and what git needs, never
  the inherited `GIT_*` or askpass variables.
- **A folder's `.git` file is read by purlis**, and git is given the git directory that was
  checked (#1055).
- **No pager**: git's output is a pipe, never a terminal, and git starts a pager only for a
  terminal.
- **No alias under a verb purlis runs**: every call names one of git's own verbs, and git never
  takes an alias of the same name in its place.
- **A call that crosses a network** (`run_network`) also holds the one-credential rule above,
  and is listed in the network log once git has run (#959): the host, the git operation and
  whether git finished it, never the repo's path, a ref or a credential. The host is where git
  sends the call, read as git reads it: a URL's rewrites applied (a push reads `pushInsteadOf`
  first), a remote's push URLs for a push, and for a push that names no remote the remote git
  pushes to (`branch.<name>.pushRemote`, then `remote.pushDefault`, then the branch's remote).
  A remote with no host (a folder) is no network call and is not listed. When the window runs it against a host you
  have a purlis account on, that one credential is your account's, through purlis's own helper
  ([forges.md](forges.md), "The git the window runs", #752); otherwise it is the forge CLI's.
- **A verb that can reach another repository runs only through `run_network`** (#1853): a
  fetch, pull, push, clone, `ls-remote`, a `remote` or `submodule` command that fetches, a remote
  archive and the transport verbs. Every other entry point refuses one before git runs, so none
  can skip the one-credential rule or the network log. A clone or push to a folder on this
  machine goes through `run_network` too, given no credential and not listed. Kept everywhere
  are the lookups that connect to nothing: `ls-remote --get-url`, and a `remote` that only reads
  or writes its own config.
- **A call that crosses a network uses only your own network settings, not a repository's.**
  A proxy, TLS and certificate settings, extra headers and the other `http.*` keys, a remote's
  own proxy, and an address rewrite (`url.<base>.insteadOf`, `.pushInsteadOf`) to another host
  or repository are yours to set, in your global or system git config. Where a repository's own
  config sets one, the runner gives git your value for it on the command line, or git's default
  where your config says nothing of it. Where neither is exact, the call is refused with a
  sentence naming the kind of setting. A few pacing keys stay the repository's (`http.postBuffer`,
  the low-speed limits, `http.version`), and a rewrite to a `file://` repository on this machine
  is kept: it crosses no network and asks for no credential.

| Entry point | For |
| --- | --- |
| `run`, `run_until` | a read with a deadline; `run_until` also hears a person's stop |
| `run_untimed` | a call that checks out a tree (`worktree add`, `merge`) |
| `run_with_input`, `spawn_fed` | a verb fed on standard input (`--stdin`, `cat-file --batch`) |
| `run_network` | `clone`, `fetch`, and `publish` when it lands |
| `run_as_session` | the project-root guards, which ask what the session's own git would do |
| `run_in_hook` | a read from inside a git hook, of the index the hook's command is using |
| `spawn_persons_clone` | the person's own clone from the welcome's Get a project (see below) |

### A git that is stopped is stopped with what it started

git works through children: a filter for each file it checks out, a credential helper, the
remote helper, a signer, and under `pull` or `submodule update` another git that writes the
working tree. So a git the runner starts leads a process group of its own, and a deadline or
a stop ends the group (`worktree::git::group`):

- **Asked first.** The group is sent `SIGTERM`, which is the signal git removes its locks on,
  and is woken in case the kernel had stopped it. It has two seconds, and they are the
  group's: a git under a `pull` may remove its lock a moment after the `pull` itself has gone.
- **Killed only if it will not go.** What is left of the group after that is killed, and git
  is waited for last, so the group's id is never signalled once it could be a stranger's.
- **No prompt is lost by it.** A group of its own is not the terminal's foreground group, but
  the runner's git never reads a terminal: its input is nothing or a pipe, its output is
  piped, git's own prompt is off, and no askpass program is passed.

The forge's CLI is started and ended the same way. A program that leaves the group on purpose
is not reached.

**The `purlis` command is the exception.** Whoever started it ends it by its process group: a
terminal's Ctrl-C, a harness ending a hook, the app ending a chat or a brokered command. So
the gits it runs stay in that group, where the same stop reaches them, and its own deadline
ends git alone, as before. A job object would be the equivalent on Windows and is not made.

### Every call site

The audit of 2026-10-12 (#810) read every `Command::new("git")` in `crates/` and
`app/src-tauri/`. **Outside test code there is none**: every git process purlis starts is
started by the runner. The modules that call it are, in `purlis-core`: alerts, briefing,
change (cmd, land, push, revert), commitguard, committedby, diffscan, dispatcharrival,
dispatchplace, doctor (git, fixes), extension briefing, files (reader, compare), firstrun,
firsttask, footerclaim, forge (pr), gitbroker, githooks, gitpolicy, gitstate, glrefresh, guest,
inventory, piececmd, pieces, planegit, planeroot, profiles, programs, provenance, renamelocal,
repocmd (clone, status, submodules, sync, unread), repoinstructions, repos, reposave, scaffold,
secrets (vaultcmd), settings (forges), standings, worktree, wscmd; in the CLI, `scan`; in the
app, autosave, gitbroker, live, planewatch and worktrees.

Not routed, each for its reason:

- **Test fixtures.** The git that builds a test's repository is not the code under test, and
  is never in a shipped build.
- **`profiles::ignore_check_with`** runs a git a test names, to drive an answer real git
  cannot give. The product calls `ignore_check`, which takes the runner.
- **Reads done in process** (gitoxide: the files view's branch status) start no git at all.
- **git a chat or you run** is not purlis's call. The sandbox and the guards hold a chat's.

`tests/it/every_git_call_goes_through_the_runner.rs` fails on a new git started outside the
runner in shipped code. A site that may stay goes on its list with the reason, and the list
may only shrink. `tests/it/a_planted_repo_config_runs_nothing.rs` plants a hook folder, an
fsmonitor, a pager, aliases, every key in the table above (a signer for each signature format,
and each trailer command spelling), the keys that cannot run and the programs `git-lfs` itself
starts in a repo's own config, runs every
entry point there, and checks that none of them ran. Its control runs plain git on the same
repo and sees them run.

### What the runner does not hold

- **A config rewritten between the runner's listing and git's own read** is read by git as it
  is then. The listing is taken, or found unchanged, before each git call; it is not a lock
  on the file.
- **Other ways for git to start a program** that the table above does not name, in a git
  newer than the one it was last checked against, are not held until the runner is checked
  against them.
- **`HOME`'s global config** stays readable, because credential helpers and your identity live
  there. A program it names is yours, not a chat's.
- **`run_as_session` and `run_in_hook`** are held to the link check (#1476): a folder whose
  `.git` file names something purlis will not follow is refused, as for every runner. Past the
  check they are not handed the git directory it read, so git finds the repository as the
  session's own git does, and a hook's `GIT_DIR` still names it.

## The person's own clone

**Get a project from your forge** (#1815) clones a URL the person pasted into a folder they
picked, while they watch it. That clone is the person's own git, run for them from the window,
so it is **not** under the one-credential rule. Over HTTPS to a host they have a purlis account
on, it signs in with that account through purlis's own helper, after an empty
`credential.helper` so no other helper is asked (#752). Otherwise it uses their own credential
helpers for HTTPS and their own SSH. It goes through the runner like every other call
(`spawn_persons_clone`), and keeps what does not depend on whose credential it is:

- the constructed environment, with no hook, no fsmonitor and no submodule fetched;
- `ext::`, `git://` and plain `http://` refused, and no askpass program;
- no prompt: git's terminal prompt is off, and ssh runs with `BatchMode=yes` unless the
  person's config names an ssh command of its own. A sign-in git would have asked for is a "no
  access" sentence with Retry instead.

No chat reaches it. Its commands are the window's alone, and the URL is refused unless it is
HTTPS or SSH, without a password in it. The clone it makes is opened through the trust
question like every other project.

## When you are not using purlis for git

The policy is applied per repo, in that repo's local config, and never to your global git
config. A repo purlis has never cloned or applied policy to is untouched — your own
day-to-day SSH setup elsewhere on the machine keeps working exactly as before.
