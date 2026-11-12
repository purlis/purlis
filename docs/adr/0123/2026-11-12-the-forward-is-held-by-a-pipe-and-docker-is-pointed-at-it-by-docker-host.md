# Amended (2026-11-12): the forward is held by a pipe, not `ssh -N`, and each run is pointed at it by `DOCKER_HOST`

Records two choices the tracer [#6266](https://github.com/purlis/purlis/issues/6266) made while it
was built, after its review. Both change words of D-6257-1 (§2) and §8. The rest of this record
stands.

## The forward's session waits on purlis, and ends the connection when purlis goes

D-6257-1 said the forward is `ssh -N -L <local>:<remote socket>`. It is
`ssh -T -L <local>:<remote socket> -- <machine> <held-by>` instead, where `<held-by>` is

```sh
exec sh -c 'read line; [ "$PPID" -gt 1 ] && kill -HUP "$PPID"'
```

and ssh's standard input is a pipe from the process that holds the forward, which never writes
to it.

**Why not `-N`.** Nothing but a kill ends an `ssh -N`. A forward is held by a process (the window,
or `purlis pool …`), and a process that goes without running its own clean-up, killed outright or
crashed, never sends that kill. The remote parity job's orphan check found `-N` forwards
outliving the processes that opened them. The forward is held in a process-wide table, so even a
clean exit drops nothing by itself.

**Why the session ends the connection, not only itself.** When the holder goes, however it goes,
its end of the pipe closes and `read` returns. If the session simply ended there, ssh on this
machine would wait for every other channel on the connection to close first, and a `docker exec`
the holder started (the link's own) is such a channel and may stay open. So the session ends its
own sshd process, its parent once `exec` has replaced the person's login shell. The connection
ends with every channel on it, and ssh here ends with it. `sh` is used for the guard because the
person's login shell may be any, and POSIX `sh`, `bash`, `zsh` and `fish` all run `exec sh -c '…'`.

**And the window ends them itself.** As the window quits, once its chats have ended, it ends every
forward it holds (`sshroute::forward::let_go_of_all`). That is the usual way a forward ends; the
pipe is the way it ends when nothing else runs.

This needs no `unsafe` code and nothing on this machine besides ssh. A parent-death signal
(`PR_SET_PDEATHSIG`) exists only on Linux and needs code run between fork and exec, and a guard
process of purlis's own beside each ssh was one more process to keep alive for the same end.

## Each run is pointed at the forward by `DOCKER_HOST`, not `--host`

§1, D-6257-1 and §8 said each run of `docker` for the pool is handed `--host unix://<local>`.
It is handed `DOCKER_HOST=unix://<local>` in the run's own environment, with `DOCKER_CONTEXT`
removed, instead.

- **It is the per-run pin a local pool already has** (`dockerpool::settings`, where a local
  pool's engine is set as `DOCKER_HOST` too), so every run of `docker` purlis makes gets it, with no
  argument list to keep in step. A `--host` would have to be added to each of the dozens of
  argument lists, and one missed would quietly reach the window's own engine.
- **Docker reads it as `--host`**: `DOCKER_HOST` names the engine exactly as the flag does, and
  takes precedence over a context. The window's own `DOCKER_HOST` and context are never used,
  which is what §1 asks for.
- It is set on the child's environment only, never on purlis's own.

## Nothing else of the person's SSH settings rides on the forward

§2 said everything other than purlis's options is the person's own SSH settings. That still holds
for where and how ssh signs in (aliases, users, ports, jump hosts, identity files). It no longer
holds for what a session does once it is in:

- `RemoteCommand=none`, `Tunnel=no`, `RequestTTY=no` and `PermitLocalCommand=no` are passed on
  the command line, where ssh takes the first value it is given, so no ssh_config's own is used.
- The forward passes `ClearAllForwardings=no`: `yes` would clear purlis's own `-L` too, since ssh
  clears forwards named on the command line with those of its settings. A sign-in, a probe and a
  first contact forward nothing and pass `ClearAllForwardings=yes`.
- **A route whose settings add a `LocalForward`, `RemoteForward` or `DynamicForward`, a tunnel, or
  a `SessionType`, `StdinNull` or `ForkAfterAuthentication` other than ssh's default is refused**,
  at Connect and before every forward opens, with a finding that names what to remove. ssh adds
  forwards from every place that names one, so none can be turned off from the command line, and
  each of the others would end the forward's session or never start it. The settings are read
  from `ssh -G`, as Connect's other checks are.
