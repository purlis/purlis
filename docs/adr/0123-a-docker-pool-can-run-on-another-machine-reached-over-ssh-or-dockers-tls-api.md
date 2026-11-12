# A Docker pool can run on another machine, reached over SSH or Docker's TLS API

**Proposed 2026-11-11** for the person's acceptance, drafted for spec #6257 by the design lane.
It follows the person's ruling of 2026-11-11: *"add Docker pools on another machine, by every
standard way Docker reaches a remote engine: SSH and the API."* Its concept is **Pool** (spec
#2041). The decisions marked D-6257-n are the lane's own, each with its reason, for the person to
accept or change.

## Where purlis was

A Docker pool runs only on this machine's own engine. `dockerpool::settings::engine` points each
run of `docker` at the window's `unix://` `DOCKER_HOST`, the system's socket or an installer's,
and only where no chat can put another socket in its place. `engine_on_this_machine` refuses
every other engine (`tcp://`, `ssh://`, any other scheme) as "an engine purlis cannot tell is on
this machine", because nothing told purlis which machine that was or that it stayed the same one.
`DOCKER_CONTEXT` is cleared and `docker context` is never followed. Pool setup (#4469) answers
such an engine with `Engine::Elsewhere`: it makes no runner there and asks it nothing. Browser
logins (`poollogins.rs`, #2561) reach only a local engine's runner or a cluster pool the person
marked. The only way to another machine is a Kubernetes cluster pool.

"Another engine" already means something in the window. It is #4166's state
`on_another_engine`: a runner was made on one engine, and Docker now reaches a different one, by
`docker info`'s `.ID`. That stays as it is. This record adds a different thing, a pool whose
engine is on another machine on purpose, named and pinned when the pool is added.

A runner links today by `docker exec -i --user <n> <container> /usr/local/bin/purlis bridge`
(`Made::connect_args`). The link is ADR 0078's encrypted link with the runner's own pinned key.
It goes over the engine's own connection, and nothing in the container listens on a port.

## The decision

### 1. A third pool kind: Docker on another machine

A pool of the kind **Docker on another machine** is a Docker pool in every way but where its
engine is. It uses the same runner image by digest, the same confinement and `check` before every
link, the same limits, disk check (#6205), stop reasons (#6181), version follow
(`pools::follow`), sign-ins, agent identity and moves. The pool record says which engine the pool
uses (§2) and the pin that holds it (§3). Each run of `docker` for that pool is handed that
engine, never the window's `DOCKER_HOST` and never a context. A local Docker pool keeps
`settings::engine` exactly as it is.

In code the kind is `remote-docker` (`poolsetup::Kind::RemoteDocker`, and `kind` in
`<config>/runners.json`). The window calls it "Docker on another machine".

### 2. The routes to an engine, and the one refused

The person picks a route at the Connect step (§6). Each is a standard way Docker or Podman
reaches a remote engine, and each ends as one engine address the pool keeps:

- **SSH** (recommended): `ssh://user@host[:port][/path/to/socket]`, Docker's and Podman's
  spelling. Without a path, purlis looks for the engine's socket on the host: the system's
  `/var/run/docker.sock`, rootless Docker's `$XDG_RUNTIME_DIR/docker.sock`, then Podman's
  rootful and rootless sockets.
- **Docker's API over TLS**: `tcp://host:2376`, with the server verified against a CA and a
  client certificate, the way `docker --tlsverify` does it.
- **A Docker context** the person already has, picked from `docker context ls`. It is read once,
  at Connect, and becomes the SSH or TLS route its endpoint names. The pool keeps that route, not
  the context's name. Later runs never read the context. If the context changes afterwards,
  doctor says so and offers to connect again (§7). A context whose endpoint is a local socket is
  offered as the local Docker kind.
- **A Podman connection**, picked from `podman system connection list`. It is read once and
  becomes the SSH route to that machine's Podman socket, with the identity file it names.

**Plain `tcp://` without TLS is refused, and so is TLS that does not verify the server.** The
guide gives the reason in one sentence: anyone who can reach that port can run anything on that
machine. It then shows the two ways forward, SSH (no change on the other machine but SSH itself)
or turning TLS on (Docker's own guide, with its commands under See what this does).

**D-6257-1: SSH reaches the engine by forwarding its socket, not by Docker's `ssh://` helper.**
purlis keeps the address in Docker's spelling. It reaches it with OpenSSH, which forwards the
remote engine's socket to a socket of purlis's own on this machine (`ssh -N -L
<local>:<remote socket>`), held for as long as the pool is in use. Each run of `docker` is then
handed `--host unix://<local>`. Four reasons:

1. purlis can pin the host key in a file of its own (§3) and pass the rest of its ssh options on
   the command line. Docker's helper takes neither, and runs `ssh` with what the person's
   settings say.
2. The pool uses one SSH connection, not one per `docker` command. Docker's helper connects anew
   for every command it runs, and a link alone runs several (`check` reads the container, its
   volume and the daemon before every link). That is a delay a person would notice on each start.
3. The other machine needs no `docker` program, only the engine and SSH. Podman's own remote
   works this way, so Podman needs no route of its own.
4. Everything after the socket is unchanged: a remote pool's `docker` reaches a `unix://` socket
   in a folder no chat may reach, as `settings::trusted` already requires.

The local socket lives in `<data>/engine-links/<pool>/`, made `0700`, held from every chat's
sandbox for reading and writing as Docker's own sockets are (D-2752-7), and removed when the
forward ends. Its row, with its tier (*Machine, device-bound, rebuildable*), goes into
`docs/plane-format.md` before the code that makes it (ADR 0069).

The ssh options purlis passes, and that nothing overrides: host key checking strict, against the
pool's pinned key alone, with the key never updated by the server; no master connection shared
with another ssh, and none opened; no password or passphrase prompt (signing is the agent's or a
key the person's settings name); the forward must succeed or ssh exits; and a keep-alive, so a
lost connection is noticed within a minute. Everything else is the person's own SSH settings, as
ssh reads them: host aliases, users, ports, jump hosts and identity files. purlis already holds
those settings from every chat's writes (`sandbox::PLANTED`).

### 3. The trust pin

**A pool on another machine is pinned when it is added**, and the pin is kept with the pool in
`<config>/runners.json`:

- **The machine**: for SSH, the host key the person confirmed at Connect. For TLS, the SHA-256
  fingerprint of the CA certificate the server was verified against.
- **The engine**: `docker info`'s `.ID`, which #4166 already keeps per runner. A Podman prints
  none, so for Podman the engine part is the socket's path and the store's graph root, as
  `podman info` says them.

**At first contact the person confirms the machine's key.** If the person's own ssh already
knows a key for the host, Connect shows that key's fingerprint as the one to confirm. Otherwise
Connect shows the fingerprint the host presents, with the command to print it on that machine
(under See what this does) so the two can be compared. Nothing is pinned until the person
presses Trust this machine. For TLS, Connect shows the CA's subject and fingerprint the same way.

**purlis keeps the pinned key in a file of its own and hands ssh only that file**, written fresh
for each forward in the run's own settings folder (`<data>/docker-settings/<run>/`, as for every
run of `docker`) and removed with it. So the key ssh trusts for a pool is the one the person
confirmed, and nothing else can change it. The TLS files are checked against the pinned CA
fingerprint before every run that uses them.

**When the pin no longer holds, the pool stops and asks.** A different host key, a server that
no longer verifies against the pinned CA, or a different engine ID:

- starts no new runner, opens no new link, and sends no vault value. A link already open stays
  open, because its runner's own pinned link key (ADR 0078) still proves which runner it is.
- shows a window **Notice** (cause key `pool-pin-changed`) naming the pool, what changed (the
  machine's key, the CA, or the engine), the old and new fingerprints, and the usual reason
  ("this happens when the machine is reinstalled or Docker is reset"). It offers two remedies:
  **Check and trust again**, which runs Connect's confirmation again and pins what the person
  confirms, and **Remove pool**.
- clears the pool's owned mark (§5), which the person sets again if they still mean it.
- for an engine ID that changed under the same machine key, says what #4166 says today: the
  runners made on the old engine are not on this one, so they are named "on another engine",
  never "removed", and Make the runner again is offered only after the person trusts the new
  engine.

`purlis pool doctor <pool>` prints the same finding with the same words, and exits 1.

### 4. Where credentials live: nowhere in purlis

purlis stores no credential for a remote engine:

- **SSH** signs with the person's agent or the key the person's SSH settings name. purlis never
  reads a private key, never asks for a password and never writes one.
- **TLS**: the CA, certificate and key are referenced where they already are (such as
  `~/.docker/` or a context's folder), and the pool keeps their paths and the CA's pinned
  fingerprint. The person may instead keep the client key in a vault entry. purlis then writes it
  for one run into that run's own settings folder (`0600` in `0700`, as Docker's settings are
  today) and removes it when the run ends.
- **A context or a Podman connection** adds nothing new: it is read into one of the two above.

What Send to your admin and Get help carry (#4469) names the route, the host and the
fingerprints, and never a key, a certificate's private half or a vault value.

### 5. The owned mark and vault logins (#2561)

A runner on another machine is not this machine, so it is not owned without a mark. The person
marks the pool owned at the end of Connect: *"Only you, and people you trust with your
logins, can sign in to this machine."* Not marked is the recommended default. The question
appears again under the pool's page. This is the cluster pool's mark (#4071) applied to this
kind:

- **A marked pool's runners may receive a vault value**: a browser login while the desktop is
  connected, under the person's opt-in, and a tool's value for one run (ADR 0078 as amended
  2026-10-26). An unmarked pool's runners never receive one, and the chat is told why.
- **The mark is bound to the pin**: it names the machine as pinned (host key or CA, and engine).
  A pin change clears it (§3). This satisfies #2561's "the owned mark can say which host a pool's
  engine really is".
- **A login's opt-in (`poollogins::Bound`) gains the pin** for this kind, in place of the local
  socket's path, so it is bound to the runner that actually receives the values.
- The local Docker kind is unchanged: a local socket purlis trusts needs no mark. A socket
  forwarded from another machine by anything other than this kind is still not taken for this
  machine.

### 6. The setup guide's steps for this kind

`Kind::RemoteDocker.steps()` is **Where → Connect → Pool → Sign-ins → Agent identity → Try
it**. Only Connect is new. The others are the Docker kind's steps, reading the pool's own engine.

- **Where** offers "Another machine with Docker" beside "This machine" (and the cluster once it
  is built). It recommends it when this machine has no Docker but the person's SSH settings,
  contexts or Podman connections name another machine.
- **Connect** asks in this order: the route, with SSH preselected; the machine (picked from the
  person's SSH hosts, contexts or Podman connections, or typed); first contact and the
  confirmation (§3); the engine check (§7); and the owned mark (§5). Its main button is **Trust
  this machine** before the pin and **Continue** after. Its Options hold the socket's path, the
  TLS files or vault entry, and the port. See what this does shows the exact `ssh` and `docker`
  commands, so an admin can run them by hand, and the next read picks the result up.
- **Pool** is the Docker kind's step, its image and limits checked against the other machine's
  architecture and disk.
- **Try it** adds one line to its checks: the link's round trip to that machine, measured.

The local kind's `Engine::Elsewhere` gains a second action: **Set up Docker on another machine**,
which opens this kind's guide with the window's `DOCKER_HOST` or current context filled in where
it is an SSH or TLS address. The first action, **Use this machine's Docker**, stays.

### 7. Doctor checks, and what a person is told

Each check is a finding of the pool setup module (#4478), read fresh, with the same words in the
window and in `purlis pool doctor`. Each says what happened and one thing to do
(`docs/ui-copy.md`):

| Check | What the person reads |
| --- | --- |
| The name resolves | "purlis couldn't find a machine called {host}. Check the name, or pick it from your SSH hosts." |
| The machine answers | "{host} didn't answer on port {port}. Check that it's on and on this network, and connect your VPN if you use one." |
| SSH accepts the sign-in | "{host} didn't accept your sign-in. purlis signs in with your SSH agent and never asks for a password. Add your key to the agent with `ssh-add`, then check again." |
| The machine's key is the pinned one | §3's Notice. |
| An engine answers at the socket | "{host} has no Docker running that {user} can use. Start Docker there, or name its socket under Options." |
| The account may use it | "{user} on {host} isn't allowed to use Docker. On {host}, add {user} to the docker group, or use rootless Docker or Podman." (the commands under See what this does) |
| Plain TCP | §2's refusal. |
| TLS verifies | "{host}'s certificate isn't signed by the CA you chose" / "has expired" / "is for another name ({name})", each with the file to check. |
| The engine is new enough and confines runners | The local kind's daemon check (`check_daemon`), naming the machine. |
| The architecture has a runner image | "{host} is {arch}, and the runner image is built for linux/amd64 and linux/arm64." |
| Disk | #6205's words, naming the machine. |
| The image can be pulled there | "{host} couldn't download the runner image. Check that {host} can reach {registry}." |
| The context or Podman connection still names the route | "Your context {name} now points somewhere else than this pool. Connect again to follow it, or keep the pool as it is." |
| The engine ID is the pinned one | §3's Notice, in the engine's words. |

### 8. The link: the same `docker exec`, over the engine's connection

**No port is opened on the other machine, and nothing in the runner listens.** The link stays
`docker exec -i … purlis bridge`. Over SSH, its standard input and output ride the forwarded
socket inside the pool's one SSH connection. Over TLS, they ride the API connection the person
already serves. ADR 0078's encrypted link, the runner's pinned link key and the pairing are
unchanged, so the engine route only carries the link. It never vouches for the runner.

What changes:

- **The engine is the pool's**, handed to each run as `--host` (§1), not found from the window's
  environment.
- **The SSH forward is part of the link's liveness.** A dropped forward drops the link, and the
  existing reconnect (ADR 0078, amended 2026-10-14) opens the forward again before it links
  again. The window shows "reconnecting" as it does today. Each attempt checks the pin first.
- **A runner keeps working while the desktop is away**, since its container is on the other
  machine, and suspends itself after its pool's time without a link (ADR 0089, amended
  2026-10-27), as a cluster runner does.

### 9. The relationship to RR-1 (#720)

RR-1 is an SSH runner without Docker: purlis's session host installed on the machine over SSH
(RR-14, #723) and run there under the Linux sandbox. **Where the machine has Docker, this kind
replaces RR-1.** It needs no install on the machine, since the runner image carries purlis, and no
bootstrap or version skew of its own (`pools::follow` already does it). RR-1 stays for machines
without Docker. It is built on this record's SSH route, host-key pin and SSH doctor checks, and on
the same CI fixture, so the two kinds share one SSH module. #4469's "SSH hosts" kind is RR-1's,
named in the window "Another machine without Docker" when it is built.

## Testing

- **The CI seam is a dockerised sshd beside Docker-in-Docker** (`docker:dind`), the sshd sharing
  dind's socket, with simulated latency and loss (QA-15, #598). dind's own TLS port and the CA it
  generates are the TLS route's test engine, and a Podman service container is Podman's. The job
  runs on Linux in CI. A parity test that runs there ends `::in_a_remote_docker_pool`, and that
  job selects it.
- **The pool setup module is tested on facts** (`poolsetup::of`, pure): each route, each doctor
  finding, and each pin change with its state and words. `purlis pool doctor` and the window are
  checked to say the same.
- **The SSH route's options are tested as the arguments purlis builds**, as `Made::connect_args`
  is, and against the sshd: a changed host key refuses, and a host key added anywhere else than
  the pool's own file is not trusted.
- **The Connect step has stories** for each state, in light and dark, with axe and keyboard
  checks. Its UI is cut only once its prototype is accepted (#4469's delivery order).

## What this changes in other records

- **ADR 0078 §2**: the container connector runs against the pool's own engine. The SSH forward is
  part of the link's liveness (§8).
- **ADR 0089, as amended 2026-10-14** (ownership replaces the label): a pool on another machine
  is owned only by the person's mark, bound to its pin (§5).
- **Spec #4469**: the steps of `remote-docker`, and the Connect step. Its Out of scope line "SSH
  hosts or any other kind of pool" now leaves out only SSH hosts without Docker (RR-1).
- **Spec #2041**: its Out of scope "SSH runners beyond what M44 already plans" now has this kind
  inside M44.1.
- **#2561**: its remaining criteria are met by §5 for this kind.
- **#720 (RR-1)**: scoped to machines without Docker, built on this record's SSH module (§9).
- **`docs/plane-format.md`**: the pool entry's route, pin and owned mark in `runners.json`, and
  `<data>/engine-links/`, each before its code.

## Considered options

- **Docker's own `ssh://` helper (`DOCKER_HOST=ssh://…`)**, the most familiar spelling. purlis
  keeps the spelling. The helper itself was not chosen for the reasons in D-6257-1.
- **Following a context at every run.** It would keep up with a person's change to the context.
  It was not chosen because purlis keeps one rule for all routes: an engine is named once, pinned,
  and changed only by the person in purlis. Doctor's check of the context does the keeping up
  (§7).
- **A link of purlis's own to the runner, such as a published port or a tunnel.** It would need a
  port opened on the other machine, and a key handled there that the engine's own connection
  doesn't need.
- **Trusting the person's `known_hosts` as is.** This is how ssh behaves normally. It was not
  chosen because a pool's trust is then whatever that file says at each connection, not what the
  person confirmed once.
- **Allowing plain TCP behind an "I understand" switch.** Refused by the ruling's own words and
  #6257's: the guide shows the two safe ways instead.

## What it costs

- purlis holds one ssh process per remote pool in use, and supervises it with the link.
- A person who changes their context or Podman connection connects the pool again to follow the
  change. purlis tells them when it no longer matches.
- A machine reinstalled under a pool stops that pool until the person checks and trusts it again.
- The remote account that reaches the engine controls that machine's Docker. The guide says so
  at Connect and offers rootless Docker or Podman under Options.

## Amendments

The amendments to this decision are files of their own in [`0123/`](0123/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record.
