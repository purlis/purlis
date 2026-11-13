# Amended (2026-11-14): a Docker context is read once, and its name is kept beside the route for doctor alone

Records the choices [#6268](https://github.com/purlis/purlis/issues/6268) made while building
the context route. They sharpen §2's "A Docker context", §6's Connect and `Engine::Elsewhere`,
and §7's check that the context still names the route. The rest of this record stands.

## What a context is read from

Connect lists the contexts with `docker context ls --format '{{json .}}'` and reads them with
`docker context inspect` of the names listed, both run with the person's own Docker settings
(where the contexts are kept), with `DOCKER_CONTEXT` and `DOCKER_HOST` cleared and Docker's
plugin hooks off. Nothing reads Docker's context store on disk. An endpoint is read into:

- `ssh://…`: the SSH route, as one typed is.
- `tcp://…` with the context's own TLS material (`ca.pem`, `cert.pem`, `key.pem` in the folder
  `inspect` names as `Storage.TLSPath`, under `docker/`): the TLS route with those paths. The
  files stay where Docker keeps them (§4 stands).
- `unix://…` or `npipe://…`: this machine. Picking it chooses the local kind.
- Plain `tcp://`, a context with `SkipTLSVerify`, TLS material without a CA, and TLS without a
  client certificate are refused, the first three in §2's words.

The window sends a context's name alone, and the core reads the contexts again before keeping
one, so what is kept is what `docker` says now.

## The pool keeps the context's name beside its route, for doctor alone

§2 said the pool keeps the route, not the context's name, and §7 asks doctor to name the
context that moved. Without the name there is nothing to compare, as the Podman amendment of
2026-11-12 found. So the pool keeps both: the route, which is all any run uses, and the
context's name (`dockerContext` in `runners.json` and `pool-setup.json`), which only Connect's
reading uses. It is no part of what defines the pool. No run follows it, and every run keeps
`DOCKER_CONTEXT` cleared. A machine typed, or TLS files named by hand, forget it.

## Doctor's check, and its two ways on

Connect reads a pool's context fresh each time the pool's setup is read. Where the context now
names another route purlis uses, Connect needs attention (cause `context-moved`) with §7's
words, and the pool's checks all still hold. Its two ways on:

- **Connect again** starts a new pool's setup with that context picked as it is now. The pool it
  came from keeps running where it does until the person removes it: a pool's route is changed
  only by making a pool there, never in place, as for every other route.
- **Keep the pool as it is** forgets the context's name on the pool, so the check stops.

A context that is gone, or that now names this machine or something refused, has nothing to
follow, and says nothing.

## Set up Docker on another machine, from the local kind

Where the local kind's engine is elsewhere, Where offers Set up Docker on another machine
beside its own words. It opens Connect with the window's `DOCKER_HOST` where that is an
`ssh://` or `tcp://` address, else with the current context where its route is one purlis uses
(that context picked, its TLS files with it). Docker follows no context while `DOCKER_HOST` is
set, so neither does this.
