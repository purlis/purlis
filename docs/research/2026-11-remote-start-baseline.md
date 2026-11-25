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

**Pending.** The numbers come from the first `remote-parity` run of
`a_start_on_a_remote_pool_times_each_of_its_steps`, which writes its table to the job's summary;
paste it here from that run. Nothing below is measured yet.

## Not measured by that test

`first_link`, `profile_ready`, `start_program_call`, `harness_spawn` and the `browser_engine_*` steps need a
whole app or a runner host with a harness, which the link-level rig does not start. Their spans
are in place; their numbers come from the first run of the e2e remote pool scenarios
(`PURLIS_E2E_REMOTE_POOL_IMAGE`) or from a person's own log. `docker` on this machine (a local
pool) passes through the same `docker_*`, `container_check`, `link_*` steps.
