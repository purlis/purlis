# Amended (2026-11-12): a Podman there is reached with Podman's own client, and pinned by its socket and store

Records the choices [#6269](https://github.com/purlis/purlis/issues/6269) made while building
Podman over SSH. They sharpen §2's "A Podman connection" and §3's engine pin. The rest of this
record stands.

## Podman's own client, through the same forward

§2 said a Podman connection becomes the SSH route to that machine's Podman socket. It does, and
the forward is the same `ssh -T -L` of D-6257-1. What reaches the socket through it is **this
machine's own `podman`**, pointed at the forward by `CONTAINER_HOST` (and `DOCKER_HOST`), with
`CONTAINER_CONNECTION` removed, never Docker's client.

**Why not Docker's client against Podman's Docker API.** The confinement check before every link
holds Podman to what `podman inspect` and `podman info` were measured to write
(`dockerpool/hosts/`, #2680). Podman's Docker API writes the same container in Docker's shape,
which no host was measured on, so the check would refuse it, or would have to be loosened to a
shape nobody measured. And that API draws a new `.ID` at every `info`, so no Docker pin could
hold. Podman's own client writes exactly the measured words. So:

- Connect asks the engine with Docker's client first. Where `docker version` names `Podman Engine`
  among the server's components, it asks again with this machine's `podman`. With no `podman`
  here, Connect says so (cause `needs-podman`) and pins nothing.
- `podman` is looked for where Podman's installers put it (`/opt/podman/bin`, Homebrew's,
  `/usr/local/bin`, `/usr/bin`), as `docker` is looked for in Docker's places.

## A rootless Podman's runtime config is read on that machine, over the pinned SSH

The check of a rootless Podman's container reads the runtime config Podman wrote for it
(`<graph root>/overlay-containers/<container>/userdata/config.json`, #2895), because rootless
Podman writes a device it was given nowhere `inspect` shows. On another machine that file is that
machine's. purlis reads it there with the pool's pinned ssh, as a sign-in runs (`test -f <path>
&& exec cat -- <path>`), and only once the path is exactly that container's own under the graph
root `podman info` names. The init that config binds (`--init`'s `catatonit`, #3702) is that
machine's too, so whether it is a file is asked there (`test -f <path>`), never here, where a Mac
has none and another Linux keeps it elsewhere. Nothing else of that machine's is read.

## The runner's seccomp profile is kept on that machine for the run

Docker's client reads `--security-opt seccomp=<file>` and sends the profile. Podman's client sends
only the path, and the engine reads it on its own machine (`config_linux_seccomp.go`, built only
for the engine). So for a Podman there, purlis keeps the profile on that machine first, over the
pinned ssh, in a folder of its own beside the engine's socket (`umask 077`, `mktemp -d`, the
profile on standard input), names that path in the run, and removes the folder once the run has
read it. The check holds the path to the profile's file name and, rootless, holds what Podman
compiled to the profile itself, as it does here.

## The pin's engine is a Docker's id or a Podman's socket and graph root

§3 said a Podman's engine part is its socket's path and its store's graph root. `runners.json`
and `pool-setup.json` keep it as `"podman": {"socket", "graphRoot"}` in place of `"engine"`,
from `podman info`'s `.Host.RemoteSocket.Path` and `.Store.GraphRoot`. Either changing, under
the same host key, is the pin change §3 describes (cause `pool-engine-changed`), in Podman's
words: its store is somewhere else now. #6270 builds the whole change flow.

## A Podman connection's key file rides beside the route

A Podman connection names the key file it signs in with. Docker's `ssh://` spelling has no place
for one, so the pool keeps it beside its route (`identity`), and every ssh for the pool is handed
it as `-i`, beside the person's agent and SSH settings. Only its path is kept, never the key
(§4 stands). A path ssh would expand is refused: one holding a `%` token, or `${`, an
environment variable OpenSSH 8.4 and later expand in a key file's path.

## Left for later

§7's check that a Podman connection still names the route is not built: the pool keeps the route,
not the connection's name, so there is nothing yet to compare. It comes with the context route
(#6268), which needs the same check.
