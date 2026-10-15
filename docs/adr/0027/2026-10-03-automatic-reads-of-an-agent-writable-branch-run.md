# Amended 2026-10-03: automatic reads of an agent-writable branch run in-process (FM-4, V88)

**The binary stays the rule for every write and every command the operator starts.**

**There is one exception.** Automatic, read-only reads of a branch an agent can write are done by
gitoxide (`gix`), not git. These are a status, a name diff, ignore checks, a folder walk, and how
many commits a branch is ahead of and behind its recorded base (FM-5, `files::ahead_behind`). The
repository's config is read once and cut down to an allow-list of the keys a read needs, so no
filter or diff driver, and no memory or size setting, survives. No git process is started.

These are reads the app makes by itself, after every write an agent makes. A git process started
there would read config the agent can write, outside any sandbox, and git config can name programs
for git to run (#810). Blanking those programs with `-c` lost a race to a config swapped mid-read. A
read that starts no git process has no such race (dispatcher decision D-88f;
`charter_core::files::status` and `files::Root`).

**The read runs in a short-lived, bounded child of purlis's own binary** (D-88h). It is the app's
executable started again, answering one question; it starts no git and no shell. The child has a
cleared environment (only `HOME` and `XDG_CONFIG_HOME`, for the operator's own git config). It stops
itself past the runner's 30-second deadline and past a 1 GiB resident-memory cap, which it checks on
its own every 10 ms; the app kills it two seconds after the deadline if it has not stopped. Its
answer is read back capped. Before D-88h the read ran inside the app with no time or memory bound:
an ignore file linked to an endless device grew the app by gigabytes in under a second, a FIFO hung
the read for ever, and gitoxide's status kept memory per untracked file on every read. Now each of
those fails the read, in the child, and the app stays as it was.

What this costs:

- **A second git implementation to keep in step.** Its answers must stay git's, and the tests
  check them on real git fixtures.
- **Racily-clean files are compared without filters.** A file whose time moved and whose size did
  not is compared as it is on disk, so a wrong "changed" mark is possible; a program running is
  not.
- **A status can show names from object stores the repository's alternates point to.** The names
  reach the operator's explorer, never the agent.
- **A process per read.** Each read starts the app's binary again, off the thread that draws; what
  that costs on a packaged build is not yet measured.
- **`gix` runs with its default features off.** It has no network, no credential helpers and no
  worktree mutation, and a test checks that no crate in the workspace turns them back on.

**New automatic reads of an agent-writable repository follow the same rule.** "A git library, unless
someone first shows a behaviour the binary cannot give" (above) is narrowed by this exception and
stands everywhere else.
