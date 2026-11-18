# Remote start: what each step costs today (baseline for #7180)

Ticket #7182, part of map #7180 ("Remote you can trust"). T5 (budgets), T6 (connection model)
and T9 (observability) wait on these numbers.

## What was added

One `tracing` span and one line per step, from `purlis_session_protocol::steps` (the browser
engine's steps use the same guard). The line reads `purlis: step <name> done after <n> ms`, with
`(pool: <alias>)` where the step knows its pool and `(parent: <step>)` where it is inside another.
**A step that is dropped without `done()` reads `refused after`**: every `?` and early return is
a refusal, and only a step closed as it was for says `done`. A miss that is the answer (an
inspect that finds no container on a normal start) is a `docker` run that was made, so `done`.

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
| `docker_info`, `docker_container`, `docker_volume`, `docker_exec`, `docker_start`, `docker_stop`, `docker_run`, `docker_image`, `docker_other` | `dockerpool.rs::spawned` | **every** `docker` CLI run, by verb: the 6 to 7 serial checks per link show as separate lines |
| `container_check` | `Docker::resumed` | the runner container's check before each link (contains the `docker_*` runs) |
| `runner_start` | `Docker::resumed` | `docker start` of a stopped runner |
| `link_check`, `link_prepare`, `link_exec_spawn` | `host/connector.rs` | each link's check, the checked copy of Docker's settings, spawning `docker exec -i … purlis bridge` |
| `link_handshake` | `session-protocol/hostlink.rs::connect_kept` | hello, encrypted handshake, session proof (both links) |
| `first_link` | `runnerlinks.rs::on_one_link_within` | a profile call's whole link: check, exec, handshake (audit B3) |
| `profile_ready` | `chats.rs` (around `on_pools.ready`) | the pool's profile check and keep, before the harness starts |
| `start_program_call` | `hostedsessions.rs::call_start` | the window's wait for `StartProgram`, over the link and relay |
| `harness_spawn` | `host/hosting.rs::start` | the host's own start of the program (on the runner for a pool) |
| `browser_engine_prepare`, `browser_engine_download`, `browser_engine_start` | `browser-engine` | engine choice and install check; the download only when none is installed; the engine's process start until it answers |

**Nested steps overlap, so a sum counts them twice.** A step inside another carries a `parent`
field (`runner_start` is under `container_check`, `browser_engine_download` under
`browser_engine_prepare`), and the test's table has an `inside` column. `container_check`
contains its `docker_*` calls (shown as `(a call)`), `first_link` contains `link_*` and the
handshake, and `profile_ready` contains a `first_link`. To total a start, add only the rows with
nothing in `inside` that are not wrappers of others, or take one wrapper alone; never every row.

## How it is measured

`a_start_on_a_remote_pool_times_each_of_its_steps` (in
`crates/purlis-core/tests/it/a_remote_docker_pool_is_reached_over_ssh.rs`) runs in the
`remote-parity` job, against `tests/remote-docker/up.sh`: `docker:dind` behind a dockerised sshd,
with `netem delay 40ms 10ms loss 1%` on the sshd's side. It reads the spans back, **asserts each
step of the link path emitted one** in the scenario it belongs to, prints the table and writes it
to the job's summary. The round trip is the median of five TCP connects to the sshd (the delay is
on the sshd's egress, so a connect is one round trip).

Scenarios: **cold** (forward not held), **warm** (forward held, engine checked < 30 s ago; a
second link straight after, as a start with a profile makes), **runner stopped** (`docker start`
first). Each is a link made as a start makes it, through the pool's forward.

I could not run Docker or the rigs here (the sandbox has no Docker and refuses socket binds), so
the numbers below are from CI.

## Baseline

CI_NUMBERS_PLACEHOLDER

## Not measured by that test

`profile_ready`, `start_program_call`, `harness_spawn` and the `browser_engine_*` steps need a
whole app or a runner host with a harness, which the link-level rig does not start. Their spans
are in place; their numbers come from the first run of the e2e remote pool scenarios
(`PURLIS_E2E_REMOTE_POOL_IMAGE`) or from a person's own log. `docker` on this machine (a local
pool) passes through the same `docker_*`, `container_check`, `link_*` steps.
