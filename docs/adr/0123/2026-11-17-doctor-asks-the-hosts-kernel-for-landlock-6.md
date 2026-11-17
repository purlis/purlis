# Amended (2026-11-17): doctor asks the host's kernel for Landlock 6, in the runner image

Records the choice that [#7084](https://github.com/purlis/purlis/issues/7084) made. It adds a row
to §7's checks and one more to Connect's (amended 2026-11-14). The rest of this record stands.

## Why a check

A runner starts every chat inside purlis's sandbox (ADR 0067 as amended 2026-11-08), and on Linux
that sandbox needs Landlock ABI 6, first in Linux 6.12 (ADR 0067 as amended 2026-11-10). A
container runs on its host's kernel. So a pool whose host is older starts no chat at all, and
before this a person found out only when their first chat was refused. Linux 6.12 stays the floor,
by the person's ruling: no weaker sandbox is offered on a runner.

## The row

| Check | What the person reads |
| --- | --- |
| The host's kernel has Landlock 6 | "{host} runs Linux {release}, and purlis's sandbox needs Landlock version 6 (Linux 6.12 or later), so no chat on this pool will start. A container runs on its host's kernel." Then the way on: on Ubuntu 24.04, `sudo apt install --install-recommends linux-generic-hwe-24.04` and a restart; on a release at 6.12 or later that still lacks it, turning Landlock on in the kernel's `lsm=` setting; anywhere else, updating the host to Linux 6.12 or later. |

Its cause is `kernel-short`. It goes to the machine's admin, since a kernel is not the person's
own account's to change.

## How it is read: the kernel's own answer first, the release second

1. **In the pool's image, before any runner is made.** Doctor runs `purlis _landlock` in a
   container of the image, with no network, every capability dropped, no new privileges, a
   read-only root and the runner's own seccomp profile. The hidden command prints the ABI the
   kernel grants, as one number. Under the runner's profile the Landlock calls go through as they
   would for a runner's chat, so the answer is the one a chat there would get.
2. **Else by the release `docker info` names** (`KernelVersion`): before 6.12 is short. Connect
   reads only this, since it runs before an image is picked, and it runs nothing on the machine.
3. **Neither said**: nothing is held up and nothing is claimed. The line stays not yet, as a disk
   that couldn't be measured does.

The other ways, and why not:

- **The release alone.** It needs no container. But a 6.12 kernel can be built without Landlock
  in its security modules, and a distribution can backport it to an older release. The release is
  a stand-in, so it is used only where the kernel can't be asked.
- **`purlis _sandbox` with an empty profile, judged by its exit status.** It works with every
  image shipped so far. But its exit status, 126, is the same for every way the wrap fails to
  start, so it says nothing of Landlock alone.
- **A runner of the pool.** None exists when a pool is added, and making one only to ask its
  kernel would leave a container and a volume to clean up on a machine that can't use them.

`purlis _landlock` is part of the runner image's contract from here on. An image built before it
prints no number, so for a pool on such an image doctor reads the release instead. That covers
the case that prompted this (Ubuntu 24.04 on its first kernel, 6.8).

## What the chat picker shows

The picker asks nothing of Docker, since it opens often. It reads what doctor last found for the
pool's machine, which is kept in the app's process (the 2026-11-14 amendment). A pool whose host
was found short is not offered on Start a chat's Runs on row, and the row says why: "A chat can't
start there: {host} runs Linux {release}, and purlis's sandbox needs Linux 6.12 or later. Pool
setup says how to update it." After the app restarts, nothing is kept until doctor runs again. A
chat started there is then refused with the runner's words (#7085). Nothing is written to disk,
so this is no store of ADR 0069's.

## What this does not cover

A pool on this machine's own Docker has no doctor yet. On Linux its kernel is this machine's,
which the new-chat picker already reads. On macOS it is the kernel of Docker Desktop's (or
OrbStack's, or Colima's) own machine. Making a runner is not refused for a short kernel, because a
host may be updated after its pool is made.
