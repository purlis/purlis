# Remote start: what each step costs today (baseline for #7180)

Ticket #7182, part of map #7180 ("Remote you can trust"). T5 (budgets), T6 (connection model)
and T9 (observability) wait on these numbers.

## What was added

One `tracing` span and one line per step, from `purlis_session_protocol::steps` (the browser
engine's steps use the same guard). The line reads `purlis: step <name> done after <n> ms`, with
`(pool: <alias>)` where the step knows its pool and `(parent: <step>)` where it is inside another.
**A step that is dropped without `done()` reads `refused after`**: every `?` and early return is
a refusal, and only a step closed as it was for says `done`. Two more words are not refusals:
`cancelled` (the person pressed Cancel) and `gave_up` (the window's wait for `StartProgram` ran
out; the runner may still start the program, and its answer goes to `late`). A step that found
nothing to do writes no line. A miss that is the answer (an inspect that finds no container on a
normal start) is a `docker` run that was made, so `done`. A runner's `container_check` that
finds Docker has no container is not a normal start: the runner was removed outside purlis
(#4149), so that check, and the link, is refused.

The line goes to the diagnostic log (`charter.<date>.log`) and nowhere else: ADR 0075 holds,
nothing leaves the machine. **It names the pool by its alias**: the person's own name for their
own pool, in their own local log, and that is a ruling, not an oversight (a host name, a user, a
path or a `docker` argument is never in a line; a `docker` run is named by its verb only, past
any `-H`, `--context` or `--host` before it). The spans are meant to stay.

The top-level steps are `info`. Each `docker` run is a line at `debug` (`RUST_LOG=purlis::step=debug`),
the calls inside the steps.

| step | where | what it covers |
|---|---|---|
| `ssh_forward_open` | `sshroute/forward.rs::open` | lock, resolve, `ssh -L` spawn, wait for the socket |
| `engine_pin_check` | `sshroute/connect.rs::engine_holds` | the `docker info` that pins the engine, through the forward |
| `docker_info`, `docker_exec`, `docker_start`, `docker_stop`, `docker_run`, `docker_create`, `docker_rm`, `docker_cp`, `docker_ps`, `docker_inspect`, `docker_volume`, `docker_network`, `docker_system`, `docker_container_<sub>` (`create`, `inspect`, `ls`, `rm`, `run`, `start`, `stop`, `exec`, `cp`, `logs`, `other`), `docker_image_<sub>` (`pull`, `load`, `inspect`, `ls`, `rm`, `other`), `docker_other` | `dockerpool.rs::spawned` | **every** `docker` CLI run, by verb (and subcommand for `container` and `image`): the 6 to 7 serial checks per link show as separate lines |
| `container_check` | `Docker::resumed` | the runner container's check before each link (contains the `docker_*` runs) |
| `runner_start` | `Docker::resumed` | `docker start` of a stopped runner |
| `link_check`, `link_prepare`, `link_exec_spawn` | `host/connector.rs` | each link's check, the checked copy of Docker's settings, spawning `docker exec -i … purlis bridge` |
| `link_handshake` | `session-protocol/hostlink.rs::connect_kept` | hello, encrypted handshake, session proof (both links) |
| `first_link` | `runnerlinks.rs::on_one_link_within` | a one-off link's whole making: check, exec, handshake (audit B3); inside `profile_ready` for a start |
| `profile_ready` | `chats.rs` (around `on_pools.ready`) | the pool's profile check and keep, before the harness starts |
| `start_program_call` | `hostedsessions.rs::call_start` | the window's wait for `StartProgram`, over the link and relay |
| `harness_spawn` | `host/hosting.rs::start` | the host's own start of the program (on the runner for a pool) |
| `browser_engine_prepare`, `browser_engine_download`, `browser_engine_start` | `browser-engine` | `prepare` is the whole prepare, written only when it refused, was cancelled or downloaded (never for the person's own Chrome or an engine already installed); the download is its child; the engine's process start until it answers |

**Nested steps overlap, so a sum counts them twice. Sum the rows with no `parent`.** A step
begun while another one runs names it as its `parent` (a `Context` carries that across the thread
or task a step hands work to; a parent that has ended is named no more): `container_check` is inside `link_check`, and so are `engine_pin_check` and `ssh_forward_open` where the link's check opens the forward (they have no parent where the caller builds the link's route first, and in the parity rig, which calls `through()` itself); `link_check`, `link_prepare`,
`link_handshake` and `link_exec_spawn` are inside a `first_link` where one made the link;
`first_link` is inside `profile_ready` where that made it; `runner_start` is inside
`container_check`; `browser_engine_download` is inside `browser_engine_prepare` (a background update's download, from `update`, has no parent: it is its own row, not part of a start). `first_link` is written for every one-off link `on_one_link_within` makes (profile keeps, a repository listing, unpair), not for starts alone: read it under `profile_ready` for a start. A `docker` call
is at `debug` and names none: it is inside whichever step made it and is never part of a sum.
`start_program_call` has no step of its own log's inside it: the host's `harness_spawn` is the
runner's own log, another sum.

## How it is measured

`a_start_on_a_remote_pool_times_each_of_its_steps` (in
`crates/purlis-core/tests/it/a_remote_docker_pool_is_reached_over_ssh.rs`) runs in the
`remote-parity` job, against `tests/remote-docker/up.sh`: `docker:dind` behind a dockerised sshd,
with `netem delay 40ms 10ms loss 1%` on the sshd's side. It reads the spans back, **asserts each
step of the link path emitted one** in the scenario it belongs to, prints the table and writes it
to the job's summary. The round trip is the median of five TCP connects to the sshd (the delay is
on the sshd's egress, so a connect is one round trip).

Scenarios: **cold** (forward not held), **warm** (forward held, engine checked < 30 s ago),
**runner stopped** (`docker start` first). Each is a link made as a start makes it, through the
pool's forward. The rig links as the app does but is not the app: `first_link` (the second link
a chat with a profile makes, audit B3) and `profile_ready` are the app crate's steps, run only
there, so they are not scenarios of this test and nothing here asserts them; `first_link`'s
parent marking is asserted at the host's connector, which is where it is made.

I could not run Docker or the rigs here (the sandbox has no Docker and refuses socket binds), so
the numbers below are from CI.

## Baseline

From `a_start_on_a_remote_pool_times_each_of_its_steps` in the `remote-parity` job
(*parity against Docker on another machine, over SSH and TLS*) of PR #7815: run
[45902462569](https://github.com/purlis/purlis/actions/runs/45902462569), job
[140191838563](https://github.com/purlis/purlis/actions/runs/45902462569/job/140191838563), head
`6c4be6f708` merged as `e6134eaf` into `main` at `b37d426c29`, 2026-11-26 22:34 UTC. The tables
are as the test printed them; only the scenario headings are made headings here.

> **The heading's *round trip 0 ms* is wrong.** The netem delay (40 ms ± 10 ms, 1% loss on the
> sshd's egress) was applied, and everything through the forward passed it. Only the test's
> round-trip probe did not, so this run states no measured round trip (below).

### Cold: first start after launch

3021 ms in all. A step with a parent, or a call, is inside another: leave it out of a sum.

| step | inside | runs | ms (each) |
|---|---|---|---|
| ssh_forward_open |  | 1 | 473 |
| docker_info | (a call) | 3 | 182, 484, 222 |
| engine_pin_check |  | 1 | 184 |
| docker_container_inspect | (a call) | 2 | 242, 222 |
| docker_volume | (a call) | 1 | 162 |
| docker_exec | (a call) | 1 | 393 |
| container_check | link_check | 1 | 1744 |
| link_check |  | 1 | 1745 |
| link_prepare |  | 1 | 1 |
| link_exec_spawn |  | 1 | 1 |
| link_handshake |  | 1 | 566 |

### Warm: a link straight after

2041 ms in all. A step with a parent, or a call, is inside another: leave it out of a sum.

| step | inside | runs | ms (each) |
|---|---|---|---|
| docker_info | (a call) | 2 | 202, 182 |
| docker_container_inspect | (a call) | 2 | 202, 182 |
| docker_volume | (a call) | 1 | 162 |
| docker_exec | (a call) | 1 | 446 |
| container_check | link_check | 1 | 1396 |
| link_check |  | 1 | 1396 |
| link_prepare |  | 1 | 1 |
| link_exec_spawn |  | 1 | 1 |
| link_handshake |  | 1 | 593 |

### Runner stopped: its container is started

2439 ms in all. A step with a parent, or a call, is inside another: leave it out of a sum.

| step | inside | runs | ms (each) |
|---|---|---|---|
| docker_info | (a call) | 2 | 262, 222 |
| docker_container_inspect | (a call) | 2 | 242, 222 |
| docker_start | (a call) | 1 | 225 |
| runner_start | container_check | 1 | 228 |
| docker_volume | (a call) | 1 | 162 |
| docker_exec | (a call) | 1 | 457 |
| container_check | link_check | 1 | 1817 |
| link_check |  | 1 | 1818 |
| link_prepare |  | 1 | 2 |
| link_exec_spawn |  | 1 | 1 |
| link_handshake |  | 1 | 570 |

### Totals

| scenario | in all (ms) | rows with no parent (ms) |
|---|---|---|
| cold | 3021 | 2970 (473 + 184 + 1745 + 1 + 1 + 566) |
| warm | 2041 | 1991 (1396 + 1 + 1 + 593) |
| runner stopped | 2439 | 2391 (1818 + 2 + 1 + 570) |

In each scenario the steps leave about 50 ms of the whole unaccounted for (51, 50 and 48 ms):
time between steps that no span covers.

### Reading

- **What dominates is the container check.** `container_check` takes 1396 to 1817 ms inside
  `link_check`. It is 6 or 7 serial `docker` calls (`docker_info` twice, `docker_container_inspect`
  twice, `docker_volume`, `docker_exec`, and `docker_start` when the runner was stopped), each
  162 to 484 ms through the forward. `link_handshake` is 566 to 593 ms. `ssh_forward_open` is
  473 ms cold. `link_prepare` and `link_exec_spawn` are 1 or 2 ms.
- **Warm is only 980 ms cheaper than cold** (3021 against 2041). Holding the forward and the
  engine's pin saves `ssh_forward_open` and `engine_pin_check` (473 + 184 ms; the cold run's third
  `docker_info`, its first at 182 ms, is the pin check's own). The rest of the difference is
  `container_check` itself, 348 ms longer cold (1744 against 1396), most of it one 484 ms
  `docker_info`. The per-link container check stays on every link, so it, not the forward, is the
  main cost. A stopped runner adds 398 ms to a warm link (2439 against 2041), of which
  `runner_start` is 228 ms.

### The round trip the run did not see

The test's heading reads *round trip 0 ms*. `tests/remote-docker/up.sh` does apply the netem:
`tc qdisc add dev eth0 root netem delay 40ms 10ms loss 1%` runs in the sshd's container, the
script runs under `set -euo pipefail`, and this job's log shows the script going on past that
line (the registry run, the images pushed, the agent started), so `tc` succeeded.

The probe is what misses it. `round_trip_ms` times `TcpStream::connect` to the route's address,
`127.0.0.1:2222`, which is Docker's port published on the CI runner's loopback. On loopback, Docker's userland
proxy (`docker-proxy`) accepts that connection on the runner and opens its
own to the container, so the connect returns before anything reaches the sshd's `eth0`, where
the delay is. The 0 ms is itself the evidence: a SYN-ACK leaving through that qdisc would be at
least about 30 ms late. The ssh session behind the forward is not affected: its packets do
reach the sshd, and its replies leave through `eth0`, delayed. Fixing the probe so it measures
a round trip that passes the sshd's `eth0` is #7832.

So the step times above were made at about 40 ms ± 10 ms with 1% loss, as the rig intends, but
this run does not show the round trip it ran at. Until the probe is fixed, read the steps
against the rig's configured 40 ms, not against the heading's 0 ms. A link of another RTT moves
each step by about (its serial round trips × the difference in RTT): each serial `docker` call
through the forward is at least one round trip, and the forward's open and the handshake are
several.

## Not measured by that test

`first_link`, `profile_ready`, `start_program_call`, `harness_spawn` and the `browser_engine_*` steps need a
whole app or a runner host with a harness, which the link-level rig does not start. Their spans
are in place; their numbers come from the first run of the e2e remote pool scenarios
(`PURLIS_E2E_REMOTE_POOL_IMAGE`) or from a person's own log. `docker` on this machine (a local
pool) passes through the same `docker_*`, `container_check`, `link_*` steps.
