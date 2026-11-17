# Amended (2026-11-14): Connect asks the engine before the pool is made, and doctor keeps what it found

Records the choices the review of doctor for a pool on another machine
([#6272](https://github.com/purlis/purlis/issues/6272)) made. They sharpen §4's words for the
machine's admin, §6's Connect and Try it, and §7's checks. The rest of this record stands.

## Connect asks the engine two of §7's checks, before any pool is kept

Trust this machine pins the machine and reads its engine's id. It then asks that engine, held to
the new pin, two of §7's checks: that it confines runners (the local kind's daemon check), and
that its platform is one the runner image is built for. Both are read with `docker info` alone,
so nothing is downloaded and nothing on the machine changes. A machine that fails either is
refused as any other finding of Trust is: nothing is kept, and the step says what was found.

The other two engine checks need the pool's image, which is picked on the Pool step after
Connect: whether the image downloads there, and whether its disk has room. They run when doctor
runs on a pool: Check again and `purlis pool doctor`. Until then, Connect's list shows their
lines as not yet. It never shows a line as passed that nothing ran.

At Connect, the image is taken to be purlis's own. So a machine whose platform has no runner
image is refused even for a person who means to use an image of their own built for it. That
is a known limit: the setup offers such a person no way past it yet.

## Doctor keeps what it found, by the machine's route, in this process

What doctor found last for a machine is kept in the app's process, by the route as Connect keeps
it, as `pinchange` keeps a changed pin. The next read of the step shows it until doctor runs
there again or the machine is met anew with Connect to. A finding read fresh while reaching the
machine is shown over it. Nothing is written to disk, so it is no store of ADR 0069's.

## Each failure is told by what failed

- An engine that doesn't answer, or doesn't say which security options it has, is silent, never
  unconfined. Only one that answered with another seccomp profile, or none, doesn't confine
  runners.
- A download of the image that fails for a full disk is the disk's finding, in #6205's words.
  Only Docker's own words name a full disk, so they are read for that alone, as #6205 reads them.
- A disk that couldn't be measured (an image with no `df`) holds nothing up, as the local kind's
  check holds nothing up. Doctor then says that it couldn't measure the disk, never that there is
  room.
- The person's own image, built for another platform than the machine's, is theirs to change.
  The way on is another image, on a new pool on the same machine, not another machine.

## What doctor offers to make room

`docker system prune` removes every stopped container on the machine, other people's and a
suspended runner's included. On a machine shared with others, doctor offers `docker system df`
to look first, then `docker image prune`, which removes only images that have no name and that
no container uses. The local kind's words (#6205) stay as they are: that machine is the
person's own.

## Words that leave the window name no file of this machine's

Send to your admin's words and Get help's opening message name no path of the person's machine.
The TLS files and the SSH key are placeholders: `<your CA file>`, `<your client certificate>`,
`<your client key>` and `<your SSH key>`. See what this does in the window keeps the person's
own paths, since they run its commands there. Get help lists the commands that only read apart
from those that change something on the machine, which are the person's to run.

Which findings go to the machine's admin is a list of causes, each one something on the machine
that the person's own account there can't change. A cause added later goes to no admin until it
is put in the list. A platform the runner image isn't built for is not on it: no admin changes a
machine's platform.

## The round trip is one docker command

Try it and doctor time a `docker` command over the pool's connection, from its start to its
answer, three times, and keep the middle one. purlis starting `docker` is part of that time, so
the line says what it measures: "A docker command on {host} answers in {ms} ms, from start to
answer". A truer round trip would need purlis to speak Docker's API itself over the forward and
over TLS, and nothing else needs that.

## "New enough" adds no check

§7's row "The engine is new enough and confines runners" names the local kind's daemon check,
which holds no version floor: what a runner needs of the engine is checked by its effects, and
making the runner refuses an engine that lacks a flag it uses. Doctor holds the remote engine
to the same check, so it adds no version check. A floor would need a number this record would
have to name and keep.
