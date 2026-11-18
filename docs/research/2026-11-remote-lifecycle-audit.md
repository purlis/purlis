# Remote pools and sessions: lifecycle audit (2026-11-18)

Read-only audit at `main` 92d19c2320, written for the /wayfinder map of the remote-pools rework. No product code changed.

The person's complaints: pool setup is unclear and buggy; starting a chat is slow; the session lifecycle (setup, running, restore after restart, reconnect) needs hardening; performance is poor; observability is poor; remote is not yet trusted for real coding.

## How to read this, and what is not here

- **Sources are code, ADRs and tests in this checkout.** Every `file:line` was read by the auditors. A `~` before a line number marks a cite that is approximate.
- **Nothing was measured.** No tracing span or timer covers any step of a chat start, and `docs/spec.md` has no performance budget for a pool start. Every latency in section B is an estimate from constants and call counts, marked "(est.)". The constants are exact.
- **The issue tracker was not readable.** The audit sandbox blocks `gh` (it cannot read its config) and direct calls to the GitHub API. The only tracker data available was the milestone counts below. So **A's "bugs in open issues" and all of F are not done**; section F says exactly what is missing and how to finish it.
- **Security.** The auditors found no undisclosed security issue. Nothing was withheld from this file.

Milestone counts at audit time (from the forge milestone listing): M44 Runner pools 324/449 closed (125 open), M44.1 SSH & provider runners 23/35 closed (12 open), M18.1 Session host follow-ups 12/41 closed (29 open).

## Four findings the rest hangs on

1. **The SSH runner cannot be added by anyone.** The install and service code exists, but no window path and no CLI command reaches it (A1).
2. **A chat start on a pool pays for a link twice, and each link first runs about seven serial `docker` commands over SSH** (B3, B6). That is the main reason starts are slow.
3. **"Try now" cannot beat a hidden two-minute backoff**, and a start during a short outage is refused instead of waiting (B.4, C.4).
4. **The whole ssh/forward/link path logs nothing**, and CI has real sshd and Docker fixtures but never breaks a link under a live chat (E.2, D.6).

---

# Part A: Pool setup flow, end to end, per pool kind

Read-only audit of the worktree at HEAD 92d19c2320. Every file:line below was read in this session.
Paths: `app/src/...` (UI), `app/src-tauri/src/...` (Tauri commands), `crates/purlis-core/src/...` (core).
ADRs: 0078 (runner behind a connector, with its 2026-10-20 amendments), 0123 (Docker on another machine).

## 0. Map of what exists

There are three pool kinds in `poolsetup::Kind` (`crates/purlis-core/src/poolsetup.rs:57-64`): `Docker`,
`RemoteDocker` and `Cluster`. Their steps (`poolsetup.rs:66-95`):

| Kind | Steps |
|---|---|
| Docker (local) | Where, Pool, Sign-ins, Agent identity, Try it |
| RemoteDocker | Where, Connect, Pool, Sign-ins, Agent identity, Try it |
| Cluster | Where, Check the cluster, Install, Pool, Sign-ins, Agent identity, Try it |

**The "SSH runner" is not a pool kind a person can add.** RR-14 (`runnerinstall`, install purlis on a host over SSH) and
RR-15 (`runnerhost`, systemd/launchd service, `purlis serve --install`, `purlis bridge --pair`) are merged as library and
runner-side code. Their only non-test callers are:
- `crates/purlis-cli/src/main.rs:1014,1133,1144` and `crates/purlis-cli/src/bridge.rs:55` (the runner side);
- `crates/purlis-core/src/hostsocket.rs:80`, `machine.rs:499` and `sandbox.rs:2750` (path helpers).

I grepped the repo. `runnerinstall::probe` and `runnerinstall::install` are called only from `crates/purlis-core/tests/it/an_ssh_runner_*.rs`.
There is no `purlis runner add` (ADR 0078 "What changes where" assigns it to RR-1) and no SSH connector on the desktop.
`Kind` has no SSH-host variant. `poolsetup.rs:67-68` says "a later kind (an SSH host, ...)".
Every desktop-to-runner connector in code is `docker exec ... purlis bridge` (`dockerpool.rs:895`), or `kubectl exec` for a cluster.
The SSH in the shipped code is only the forward to a remote Docker socket (`sshroute`).
So section 3 below is "what the person can do today": nothing in the window, and no CLI command.

Entry points into setup (all lead to one tab, `PoolSetupTab`):
- Palette "Add a pool…" (`actions.ts:2765`) and "Add a cluster pool…" (`actions.ts:2769`).
- Settings › This machine › Pools: "Add a pool…" and "Add a cluster pool…" buttons (`settings/pools.tsx:1013-1014`).
- Settings › Runs on: "Add a pool…" (`settings/runsOn.tsx:178`).
- A pool row's "Open setup" (`pools.ts:296`, `goOnWithPoolSetupOf`).
- `AddDockerPoolHost` (`AddDockerPool.tsx:75-95`) sends all of them to Pool setup when a project is in front. Otherwise it draws the old dialog (section 5).

---

## 1. Local Docker pool

| # | Person does | UI component | Tauri command | Core code | What the person sees when it fails |
|---|---|---|---|---|---|
| 1 | Opens Pool setup (every 5 s while Where or Pool is "attention", `PoolSetupTab.tsx:447-456`; also on window focus, `:432-442`) | `PoolSetupTab.tsx` `PoolSetupGuide` | `pool_setup` (`poolsetuptab.rs:354`) | `poolsetup::resumed` -> `engine_of` (`poolsetup.rs:1685-1692`) | Core read fails: Notice "purlis couldn't read Pool setup" + "Check again" (`PoolSetupTab.tsx:476-488`, cause `pool-setup.read`). Windows: the text is "purlis keeps Docker pools on macOS and Linux only." (`dockerpools.rs:25`) |
| 2 | Where: sees This machine / Another machine (`PoolSetupTab.tsx:165-217`) | `Where` | none for reading | `where_reading` (`poolsetup.rs:626-677`) | **No docker command:** "You need Docker first. It runs your chats in a container on this machine, apart from your files." + "Open Docker's install page". **Docker not answering:** "Docker needs to be running. purlis found it, and it isn't answering. If it is running, check that your user may use it and that DOCKER_HOST, if you set it, names a socket on this machine." + "Open the Docker app" (`pool_setup_open_docker`, `poolsetuptab.rs:2284`). **DOCKER_HOST/context elsewhere:** "Docker here points at an engine purlis can't tell is on this machine, so purlis makes no runner there and asks it nothing." + no button, only the "Set up Docker on another machine" prefill (`PoolSetupTab.tsx:256-265`). Rechecks every 5 s, "Checking again by itself every few seconds." |
| 3 | "Continue with Docker" | `Where` `onChose` (`:581-588`) | `pool_setup_choose_docker` (`poolsetuptab.rs:430`) | `poolsetup::choose_kind` (`poolsetup.rs:2174`) writes the setup choices in `<config>` | Notice "That didn't work" (`PoolSetupTab.tsx:519-528`, cause `pool-setup.where`, used for every step's Tauri error) |
| 4 | Pool: types name, image, Options (limits) | `poolsetup/PoolStep.tsx` | `pool_setup_name` (`:1196`) keeps the name as typed (`choose_name`, `poolsetup.rs:2579`); `pool_setup_command` (`:1272`) for "See what this does" | `Asked::pool` (`asked.rs:117-155`), `Asked::command` | Inline refusals: "Give the pool a name first." / `IMAGE_NOT_NAMED` / "A limit under Options isn't a whole number." (`PoolStep.tsx:441-444`). Core validation says e.g. `"x" is not a number of CPUs; write one such as 2 or 1.5, to the thousandth` (`asked.rs:97-104`), `"x" is not a time; write a whole number followed by h, m or s, such as 12h or 90m` (`asked.rs:86-93`) |
| 5 | "Create the pool" | `PoolStep.tsx:440-462` (`create`) | `pool_setup` again, then `pool_setup_create` (`poolsetuptab.rs:1288`), then `pool_setup_pool` (`:1215`) | `asked::create` (`asked.rs:468-503`): `remote_of` (none for local), `contract_problems` (`:486-495`, only if image is not purlis's own), `pools::add` (`pools.rs:54-101`): `record_pool_locked`, `docker.make_for` (`dockerpool.rs:3244`: `check_daemon`, name collisions, `has_room`, seccomp profile), `docker.pair`, `keep_for_its_pool`. Any failure removes the container and volume and forgets the pool | Busy text: "Making the pool and its runner. The first time, this downloads the runner." (`PoolStep.tsx:554-556`). Error text is `crate::dockerpools::refused_said` (`devices::shown_safely`), shown in a trouble card. Image breaks the contract: card "This image is missing N things a runner needs" with the core's sentences (`PoolStep.tsx:560-585`). Name taken: "a pool or a runner named X is on this machine already; name the pool otherwise" (`runners.rs:827-830`). Container exists: "a container named purlis-runner-X is already there; remove it, or name the runner otherwise" (`dockerpool.rs:3279-3283`). Pull failure/`own_time_said` adds "This pool has its own Suspends after ... a runner image whose purlis is older ... refuses it" (`pools.rs:106-115`) |
| 6 | Sign-ins | `SignInsStep.tsx` | `pool_setup_sign_ins_by_hand`, `pool_setup_wake`, profile commands | `sign_ins_reading` (`poolsetup.rs:825-946`) | "purlis couldn't ask the runner of pool X how its sign-ins stand." + Check again; "The runner of pool X isn't running ..." + Wake the runner; tainted runner text + "Remove the pool" |
| 7 | Agent identity | `IdentityStep.tsx` | identity commands | `identity_reading` (`poolsetup.rs:964-1030`) | "purlis can't set up {host} by itself yet: one-click setup works for github.com ..." etc. |
| 8 | Try it | `TryStep.tsx` | `pool_setup_try` (`poolsetuptab.rs:1656`), `pool_setup_try_cancel` (`:1707`) | `poolsetup::trying`, `try_reading` (`poolsetup.rs:1073`) | Not walked in depth. A refused try shows a trouble card; there is a Cancel |
| 9 | Pool setup stops (removal) | `settings/pools.tsx` Remove dialog | `remove_docker_pool` (`dockerpools.rs:~585-640`) | `pools::remove_discarding` (`pools.rs:158-`) | Refuses "N of pool X may hold chats' conversations that you were not asked about, so nothing was removed. Remove the pool again to see it" (`pools.rs:184-189`) |

Failure states after the pool exists (`pool_reading`, `poolsetup.rs:748-823`):
- `Runner::None`: "Pool X has no runner yet. If it is being made, this clears by itself. If not, remove the pool in Settings › Pools and add it again." Only a "Check again" button.
- `Runner::Gone`: "The runner of pool X was removed outside purlis." + "Make the runner again".
- `Runner::OnAnotherEngine`: "The runner of pool X was made on another Docker than this one." + "Switch back".
- `Runner::OnlyDraining` and `Runner::CannotTell(why)` are covered as well.

Window-wide Notices for a made pool: `RunnerKeyChangedNotice` ("Runner key changed"; "Remove its pool and add it again to run them there."), `RunnerGoneNotice` ("Make the runner again"), `RunnerLinkDownNotice`
(label "Pool X's runner is not linked", fix "Try now", link "Check pool X", `RunnerLinkDownNotice.tsx:49-56`, copy in `downSaid`, `:60-66`).

---

## 2. Remote Docker over SSH (ADR 0123)

Also covers Docker's TLS API (`tcp://`), a Docker context and a Podman connection. These are routes of the same Connect step.

| # | Person does | UI | Tauri command | Core | What the person sees when it fails |
|---|---|---|---|---|---|
| 1 | Where: picks "Another machine with Docker" -> "Set up Docker on another machine" | `PoolSetupTab.tsx:187-193`, `onChoseRemote` (`:554-568`) | `pool_setup_choose_remote_docker` (`poolsetuptab.rs:452`) or, with a prefill from `DOCKER_HOST`/context, `pool_setup_machine`/`pool_setup_docker_context` | `choose_kind` (`poolsetup.rs:2174`), `remote_where_reading` (`poolsetup.rs:681-709`) | No local `docker`: "purlis reaches Docker on another machine with Docker's own command on this one, and finds none here. Install Docker, or only its command, then purlis checks again by itself." |
| 2 | Connect: picks host from the list, types `ssh://user@host[:port][/socket]`, picks a Podman connection or Docker context, or chooses TLS | `poolsetup/ConnectStep.tsx` (`Pick`, `TlsPick`, `RouteChoice`) | `pool_setup_ssh_hosts` (`:601`), `pool_setup_podman_connections` (`:629`), `pool_setup_docker_contexts` (`:667`), `pool_setup_machine` (`:499`), `pool_setup_docker_context` (`:708`), `pool_setup_tls_files` (`:980`) | `sshroute::hosts_here` (`sshroute.rs:1017`), `dockercontext::contexts_here`, `choose_machine` (`poolsetup.rs:2187-2222`), `choose_docker_context`, `choose_tls_files` | The three list reads **swallow their errors**: nothing is shown if they fail (`ConnectStep.tsx:797-806`). Keeping the route: "That didn't work" card (`:961-965`) with: "that address is too long to be a machine's" (`poolsetup.rs:2194`); "Your Docker has no context named X now. Pick it again from the list." (`poolsetuptab.rs:719-723`); "purlis found no docker on this machine." |
| 3 | "Connect to <host>" (reads, contacts, signs in with nothing) | `ConnectStep.tsx:838-846` (`connect`), Busy "Asking {host} for its key. This only reads." (`:~1010`) | `pool_setup_connect` (`poolsetuptab.rs:1036`) | `connecting::meet_chosen` -> `sshroute::connect::{resolve, reachable, meet, greets_as_ssh}` (`connect.rs:276-410`), `ssh` run bounded by `RUN_WITHIN` 30 s (`sshroute.rs:48,927-975`) | Findings, in the core's words (`connecting.rs:664-960`): see table below |
| 4 | Checks the fingerprint it shows, types/confirms it, "Trust this machine" | `ConnectStep.tsx:866-880` (`trust`) | `pool_setup_trust` (`poolsetuptab.rs:1070`) | `connecting::trust_chosen`, `connect::probe` (`connect.rs:440-505`: sign in with the pinned key, find the socket), `engine_pin` (`:507-540`), `engine_holds` | Same findings. On a key that differs from what was shown: `KeyChanged` ("{host} shows a different key than the one you trusted (...)") |
| 5 | "Who can sign in there" (owned) | `ConnectStep.tsx:896-912` | `pool_setup_choose_owned` (`poolsetuptab.rs:535`), or `set_remote_pool_owned` for a made pool | `choose_owned` (`poolsetup.rs:2258`) | "That didn't work" card; refused where the pin shown is no longer the trusted one |
| 6 | Pool: name, image, limits, "Create the pool" | `PoolStep.tsx` (same as local; the Connect machine fills `on`/`trust`/`tls`) | `pool_setup_create` (`:1288`): `over_tls` also emits `tls-pool-added` (`:1324-1334`) | `asked::create` -> `remote_of` (`asked.rs:262-330`: reuses the Connect pin, or refuses with the fingerprint "to check and give"), `sshroute::pool::docker_of` (opens the SSH forward, `connect::engine_holds`), `contract_problems` on the remote engine, `pools::add`, then `forget_machine` | Without trust: "{host} shows the key SHA256:... Check it on {host}: ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub prints it there. If it is the same, run this again with --trust SHA256:..." (`asked.rs:347-356`). Plain `tcp://` with no files: `PLAIN_TCP` "purlis doesn't connect to Docker over plain tcp://: anyone who can reach that port can run anything on that machine. Use ssh:// instead, or turn on TLS for Docker there." (`sshroute.rs:~225`) |
| 7 | After: doctor "Check again" | `ConnectStep.tsx:848-857` | `pool_setup_check_again` (`poolsetuptab.rs:393`) -> `sshroute::pool::look_again()`, `poolsetup::doctored` | `poolsetup/doctoring.rs` | Findings below plus Architecture, DiskFull, KernelShort, ImageUnreachable, Unconfined, ContextMoved |
| 8 | Pin changes later ("Check and trust again", "Remove pool…") | `PoolPinChangedNotice.tsx:82-101` | `pool_setup_check_and_trust_again` (`:1108`), `pool_setup_trust_again` (`:1140`) | `connecting::{meet_pool,trust_pool}` | `change.title` + "Check and trust again" / "Remove pool…" |
| 9 | Removes the pool | `settings/pools.tsx` | `remove_docker_pool` (`dockerpools.rs:~585`), `forget_docker_pool` (`:~650`) | `forget_where_untrusted`, `unreachable_of` (`dockerpools.rs:694-700`) | Unreachable machine: Notice "pool-unreachable:X" with "Forget it here" (`pools.tsx:714-726`). Copy: "{why} Nothing was removed. Forget it here forgets pool X on this machine without contacting {host}. Its runner's container and home volume stay on {host}. {host} still lists this machine as the runner's peer. purlis then says the commands that remove them on {host}. There is no undo." |

SSH hardening (`sshroute.rs:549-616`): `StrictHostKeyChecking=yes`, `GlobalKnownHostsFile=/dev/null`, `UpdateHostKeys=no`, `BatchMode=yes`,
`PasswordAuthentication=no`, `ConnectTimeout=10`, `ServerAlive*`. First contact uses `accept-new` into a scratch file (`:681`).
A person's `LocalForward`/`RemoteForward`/`DynamicForward`/`Tunnel`/... in `ssh -G` is refused with `carried_refused` (`connect.rs:~230-245`).

Findings of the SSH route and their exact copy (`connecting.rs`):

| Finding | Copy (abridged) | Next |
|---|---|---|
| NoSuchName | "purlis couldn't find a machine called {host}. Check the name, or pick it from your SSH hosts." | Change the machine |
| NoAnswer | "{host} didn't answer on port {port}. Check that it's on and on this network, and connect your VPN if you use one." | Check again |
| NotSsh | "{host} took the connection on port {port} but didn't answer as SSH{said}. Check that SSH is running on {host} and listening on that port." | Check again |
| NotMet / NotMetThrough | "purlis reached {host} but couldn't save its host key ... Run the command under See what this does ..." / "... through the jump host or proxy your SSH settings name for it ..." | Check again |
| KeyChanged | "{host} shows a different key than the one you trusted ... purlis connects to nothing there until you check it and trust it again." | Check and trust again |
| SignInRefused | "{host} didn't accept your sign-in. purlis signs in with your SSH agent and never asks for a password. Add your key to the agent with `ssh-add`, then check again." | Check again |
| NoEngine / NotAllowed | "{host} has no Docker running that {user} can use. Start Docker there, or name its socket in the address ..." / "... isn't allowed to use Docker ... add {user} to the docker group, or use rootless Docker or Podman" | Check again |
| EngineSilent | "Docker on {host} didn't answer through SSH{said}. Check that Docker is running there, then check again." | Check again |
| ForwardingRefused | "The SSH server on {host} doesn't allow forwarding ... (`AllowTcpForwarding`, `AllowStreamLocalForwarding` and `DisableForwarding` in sshd_config, or `restrict` and `no-port-forwarding` on your key)" | Check again |
| EngineChanged | "The Docker engine on {host} isn't the one you trusted: it was reset or reinstalled. ... Runners made on the old engine aren't on this one." | Check and trust again |
| TLS family | TlsFile, CaChanged, NotSignedByCa, ServerExpired, WrongName, ClientRefused, ClientExpired (`connecting.rs:783-863`) | |

Link reliability: ADR 0123 amendments of 2026-11-17 (a forward that went silent is opened once more before a start refuses, a forward's lock never outlives its ssh), `behaviour/ssh_reconnect.rs`, and #7116. The 2026-11-18 commit cf302a779e added `RunnerLinkDownNotice` with the host's reason.

---

## 3. "SSH runner" (RR-1/RR-14/RR-15): no desktop path

| Step | UI | Tauri | Core | Failure |
|---|---|---|---|---|
| Add an SSH host as a runner | none | none | none (`Kind` has no variant) | no entry point, so nothing to fail |
| Install purlis on the host over SSH | none | none | `runnerinstall::{probe,install,prune}` (`runnerinstall.rs:731,750,863`) test-only | n/a |
| Keep its host running | none | none | `purlis serve --install` is runner-side only (`crates/purlis-cli/src/main.rs:127-145`) | n/a |
| Pair | none | none | `purlis bridge --pair` is the runner's half (`bridge.rs:73-106`) | n/a |

`docs/adr/0078/...` and `runnerinstall.rs` talk of "a runner reached over SSH" as if it were a feature.
Whoever sets up a bare host over SSH gets no window path, no `purlis` subcommand and no doctor copy.
`purlis doctor` does read the runner host's service when run on the runner (`doctor/runner_host.rs`), which is the only visible piece.

---

## 4. Cluster (Kubernetes) pool: `kubectl`, `crates/cluster-doctor`

| # | Person does | UI | Tauri | Core | Failure |
|---|---|---|---|---|---|
| 1 | Where: picks a kubeconfig context (radio list, recommended one marked) | `Where` + `ContextOptions` (`ClusterSteps.tsx:34-60`), `onChoseCluster` (`PoolSetupTab.tsx:571-578`) | `pool_setup_choose_cluster` (`poolsetuptab.rs:475`) | `choose_cluster` (`poolsetup.rs:2421`), `cluster::where_reading` (`poolsetup/cluster.rs:651-`) | No kubectl: "A cluster is reached through kubectl, with your own sign-in, and {why}. You can use Docker on this machine instead." + "Open kubectl's install page". Contexts unreadable: "purlis couldn't read your kubeconfig's contexts: {why}". No context chosen: "Choose a context under Options first." (`PoolSetupTab.tsx:154`) |
| 2 | Check the cluster | `CheckStep` (`ClusterSteps.tsx`) | `pool_setup` + `pool_setup_check_again`; `pool_setup_get_help` (`:777`); `pool_setup_ask_admin` (`:857`) "Send to your admin" | `poolsetup/doctoring.rs`, `cluster-doctor` checks (`lib.rs`: version >= 1.36 `MINIMUM`, CRDs `AGENT_SANDBOX_DEFINITIONS`, `CLUSTER_SCOPED` + `ROLE_RIGHTS` via `SelfSubjectAccessReview`); reads are bounded by `READ_WITHIN` 30 s (`poolsetup/cluster.rs:36,560`) | A cluster that cannot install: "Waiting for your admin" Say (`ClusterSteps.tsx:108`) with words to send, rechecked every 30 s (`PoolSetupTab.tsx:459-465`) |
| 3 | Install: Upstream, Definitions, Signing key, Operator, each on its own button | `InstallStep` | `pool_setup_install(part)` (`poolsetuptab.rs:556`), `pool_setup_operator_image` (`:958`) | `install_part` (`poolsetup.rs:2438-2472`) -> `install::apply` (`install.rs:323-`: `kubectl apply --server-side -k`) | "kubectl {verb} said: {stderr}" (`install.rs:336-340`); "nothing was applied: choose your team's cluster on Where first" |
| 4 | Pool: make the cluster's Pool ("Create the pool"), or type/pick the existing Pool, "Read the key" | `ClusterPoolStep` (`ClusterSteps.tsx:~395-665`): `create` (`:436-452`), `check` (`:464-478`) | `pool_setup_cluster_pool` (`poolsetuptab.rs:579`), `cluster_pool_key` (`clusterpools.rs:480-505`) | `create_cluster_pool`, `pool_pin`, `key_shown` (`clusterpools.rs:~455-475`) | "This build of purlis names no runner image, so give one by its digest under Options." Pool unreadable: kubectl's words. "Type the pool's name your cluster's admin gave you, then read its key." |
| 5 | Pastes the admin's operator key, "Pin the key and add" | `KeyToCheck`, `claim` (`ClusterSteps.tsx:488-515`) | `add_cluster_pool` (`clusterpools.rs:507-545`) then `pool_setup_pool` | `pools::add_cluster` (`pools.rs:2439-2533`): re-reads the Pool, compares pin, `confirm_key`, `claim_and_pair` (`clusterpool.rs:2337-2397`): claim, wait for the operator to vouch (default 5 min, `VouchWait`, `poollimits.rs:321-332`), `kubectl exec ... bridge --pair`; any failure deletes the claim and forgets the pool | Busy "Claiming your runner. This can take a few minutes." Wait ran out: "the cluster operator did not vouch for the runner of claim ns/name within 300 seconds, so purlis deleted the claim. The claim says: ... it took longer than this pool's limit of 5 minutes (Longest wait for a cluster runner to be vouched for): raise it to 10 minutes" (`clusterpool.rs:2418-2424`, `poollimits.rs:630-635`). Key mismatch: "The key you pasted isn't the one pool X names, so nothing was pinned. Stop here and check with your cluster's admin: you may be reaching another cluster." (`pools.ts:~357`) |
| 6 | Sign-ins on a pool not marked as one's own | `MarkAsOwn` (`PoolSetupTab.tsx:304-339`) | `set_cluster_pool_owned` (`clusterpools.rs:630`) | `SignIns::NotYours` (`poolsetup.rs:881-895`) | "Pool X isn't marked as your own, so purlis keeps no profile of yours on its runner. Your cluster's admins, and whoever holds its nodes, can read what a runner holds. Mark it on its row once you accept that." |
| 7 | Joining a pool a teammate made | | `pool_setup_pin_again` (`:761`) | `pools::pin_again` | "Check key again" card |

---

## 5. Legacy dialog (no project open): `AddDockerPool.tsx`

`AddDockerPoolHost` shows `AddDockerPool` or `AddClusterPool` only when no project can hold the tab (`:96`, `if (!pools.adding || toPoolSetup) return null`).
- Kinds: Docker and Cluster only (`KindRow`, `AddDockerPool.tsx:130-160`), with no remote Docker.
- Docker dialog -> `add_docker_pool` (`dockerpools.rs:519`) -> `add_on` (`:537-545`) -> `pools::add`.
- Trouble copy: "Nothing was added: memory, processes, /tmp, frames and width are whole numbers." (`:~290`), then the core's refusal in `<p className="trouble" role="alert">` (`:412`).
- The cluster dialog uses `RaiseLimit` (`AddDockerPool.tsx:646`) to offer a longer vouch wait.

---

## 6. Bugs, dead ends and gaps (each verified in the file named)

Severity: H = person is stuck or misled, M = confusing/inconsistent, L = polish.

**A1 (H) No way to add an SSH-host runner, though the repo and ADR 0078 present it as built.**
`runnerinstall::install`/`probe` and `runnerhost` have no callers outside tests and the runner binary (section 3). `Kind` (`poolsetup.rs:57-64`) has no variant,
and the CLI has no `runner` subcommand (`crates/purlis-cli/src/main.rs`; the `Serve`/`Bridge` variants at `:127-190` are the runner's half).
A person with an SSH-only box cannot proceed, and nothing in the UI says so.

**A2 (H) Cluster path in Pool setup cannot name a kubeconfig, and `KUBECONFIG` is deliberately dropped.**
`CLEARED = ["KUBECONFIG"]` (`clusterpool.rs:872`) and `kubeconfig: null` (`ClusterSteps.tsx:497`), so every Pool-setup read and add uses `~/.kube/config` (`clusterpools.rs:128-145`).
People whose contexts come from `KUBECONFIG`, kubie/direnv or multiple files see an empty list, or "purlis couldn't read your kubeconfig's contexts", with no field to point elsewhere.
The no-project dialog has the field (`AddDockerPool.tsx` cluster form); the tab does not. `PATH` for credential plugins is also restricted to kubectl's folder and system folders (`clusterpool.rs:838-842`),
so exec-credential plugins under `~/.local/bin`, asdf or mise fail with a kubectl error.

**A3 (H) Dead end when the cluster vouch wait runs out in the tab.**
The error text tells the person to "raise it to 10 minutes", but `claim` always sends `vouch_wait: null` (`ClusterSteps.tsx:501`) and shows only `refused.words` (`:504`), dropping `refused.limit`.
The pool has just been forgotten by `forgotten_cluster` (`pools.rs:2537-2546`), so there is no Settings row on which to raise it.
The no-project dialog (`AddDockerPool.tsx:499-510,646`) offers `RaiseLimit`. Result: in the tab, a cluster that needs more than 5 minutes (new node plus image pull) can never be added.

**A4 (M) kubectl runs in the add and install path are unbounded.**
`Kubectl::time_left` returns `None` unless `within`/`all_within` was set (`clusterpool.rs:928-935`). The reads in Pool setup set it (30 s, `poolsetup/cluster.rs:36,560,1083`).
But `install::apply` (`install.rs:323-343`, `kubectl apply --server-side -k`), `create_cluster_pool`, `cluster_pool_key` (`clusterpools.rs:480-505`), `add_on` (`:~555-600`: `pool_pin`, `namespace`),
and `add_cluster`'s `context_kept`, `plugins_pinned`, `whoami` and `claim_marked` (`pools.rs:2451-2490`, `clusterpool.rs:2359-2360`) have no timeout and no Cancel.
A cluster that does not answer, or an exec-credential plugin waiting on a browser sign-in, leaves the spinner "Reading the cluster operator key the pool names." (`ClusterSteps.tsx:470`) forever.

**A5 (M) Legacy "Add a pool" dialog skips the image contract check.**
`asked::create` runs `contract_problems` for non-purlis images (`asked.rs:486-495`). `add_docker_pool`->`add_on` (`dockerpools.rs:519-545`) calls `pools::add` directly,
so a team image that breaks `docs/runner-image.md` gets a container, then fails later or oddly. Its trouble is also hand-drawn `<p className="trouble" role="alert">` (`AddDockerPool.tsx:412`).

**A6 (M) Remove on a remote pool is a dead end for several findings.**
`unreachable_of` returns a "Forget it here" answer only for `NoSuchName`, `NoAnswer` and `NotMetThrough` (`connecting.rs:640-650`, `dockerpools.rs:694-712`).
For `SignInRefused` (key left the agent or was revoked), `EngineSilent` (Docker stopped or uninstalled), `ForwardingRefused` or `NoEngine`, `remove_docker_pool` continues to `as_the_person_was_told(&machine, ...)` (`dockerpools.rs:606`),
whose `machine.for_pool(name)?` (`runnerlinks.rs:100-104`) returns the finding as a bare string, e.g. "Docker on {host} didn't answer through SSH. Check that Docker is running there, then check again."
The row offers no Forget it here. The pool cannot be removed from the window while that machine is permanently changed.

**A7 (M) Container and volume names carry no desktop id, so two desktops on one remote engine collide.**
`Runner::container()` is `purlis-runner-<pool>` and `volume()` is `purlis-runner-<pool>-home` (`dockerpool.rs:695-702`).
The second desktop to add a pool with the same name gets "a container named purlis-runner-X is already there; remove it, or name the runner otherwise" (`dockerpool.rs:3279-3283`).
The text invites removing someone else's runner. The container is labelled with `desktop_label` (`runners.rs:773`), but the message does not use it to say "made by another desktop".

**A8 (M) Errors of the first-start path are swallowed.**
- `startANewPoolSetup` ignores a command error and treats it as "not continuing" (`pools.ts:257-260`). `goOnWithPoolSetupOf` ignores its result entirely (`pools.ts:296-299`). `AddDockerPoolHost` also ignores `poolSetupChooseCluster(null)` (`AddDockerPool.tsx:~87`).
  If `config_root()` fails ("There is no config home on this machine to keep a pool in.", `poolsetuptab.rs:348`) the tab still opens and only the next read shows the Notice.
- Connect's three list reads (`ConnectStep.tsx:797-806`): a failure leaves the lists empty, and the person is not told why no hosts, Podman connections or contexts appear.

**A9 (M) `hosts_in` ignores `Include`.** `sshroute.rs:991-1011` reads only `Host` lines of `~/.ssh/config` (and `hosts_here`, `:1017-1030`, only that file).
Hosts defined in included files (the common layout with OrbStack, Colima, Tailscale or work configs) are missing from the picker. They can still be typed, but nothing says so.

**A10 (L) `where` check in `create` silently re-reads.** `PoolStep.tsx:449-454`: if Where is no longer "done" when Create is pressed, it calls `changed()` and returns with no message. The person sees the page change under them and no explanation.

**A11 (L) One shared cause key and title for every Tauri error in the tab.** `PoolSetupTab.tsx:519-528` shows "That didn't work", cause `pool-setup.where`, for errors from any step, including Connect and Install.
There is no per-step "what happened / what to do", although `docs/ui-copy.md` asks for it, and `Notice.guard.test.ts` cannot tell.

**A12 (L) No Cancel on long synchronous Tauri calls.**
`pool_setup_connect` can run `ssh -G`, a greeting probe, a first contact and a key read, each bounded at 30 s (`sshroute.rs:48`, `connect.rs:276-410`) but with no overall limit or Cancel (`ConnectStep.tsx:838-846`).
`pool_setup_create` pulls the runner image with no timeout (`dockerpool.rs:3316-3322`, final arg `None`) and no Cancel ("Making the pool and its runner. The first time, this downloads the runner.").
Only Try it has a Cancel (`pool_setup_try_cancel`, `poolsetuptab.rs:1707`).

**A13 (L) Windows is not gated in the UI.** Every `pool_setup*` command returns `NOT_HERE` on non-unix (`poolsetuptab.rs` passim, `dockerpools.rs:25`), yet the palette's "Add a pool…" (`actions.ts:2765`) and the Settings buttons are offered.
The person sees "purlis couldn't read Pool setup / purlis keeps Docker pools on macOS and Linux only." with a "Check again" button that cannot help (`PoolSetupTab.tsx:476-488`). The spec lists Windows only as a later milestone (`docs/spec.md:651`), so this matters only when it ships.

**A14 (L) Where's "Elsewhere" state has no direct button.** `Engine::Elsewhere` -> "Docker here points at an engine purlis can't tell is on this machine ..." with action `UseLocalDocker` (`poolsetup.rs:655-664`),
but `Where` renders `onlySaid` (`PoolSetupTab.tsx:219,256-265`): no button, only the remote prefill and "Check again by itself". The remedy ("Switch your Docker context or DOCKER_HOST to this machine") is not applied for the person.
Works as designed (ADR: nothing changes but on a step's button), but it is a place where the person is told to leave the app.

---

## 7. Recent fixes found by git log (so you do not re-report them)

- #7130 (11-17) forward's lock never outlives ssh; #7122 forget a pool whose machine does not answer; #7084 Landlock 6.12 check; #7116 remote pool comes back after the machine restarts; #6955 Connect distinguishes "not SSH" from "key not saved" and uses one greeting deadline; #6823 Connect works with a space in the path; #6463 forwarding refused is named; #6205 full disk is plain.
- The 2026-11-18 ADR 0078 amendment ("a runner link down past a blip says why and offers Try now") corresponds to `RunnerLinkDownNotice`.

## 8. Not walked in depth

Sign-ins, Agent identity and Try it copy (`SignInsStep.tsx`, `IdentityStep.tsx`, `TryStep.tsx`), Docker TLS-file entry (`TlsPick`), and `purlis pool` CLI parity (`crates/purlis-cli/src/pool.rs`, `poolsetup.rs`).
The cluster-doctor check list was read only at its constants.

## Top problems found

1. SSH-host runner (RR-1/14/15) has no add path in the window or CLI: only library and runner-side pieces exist (A1).
2. In the Pool setup tab the cluster vouch wait cannot be raised, and the failed add forgets the pool, so slow clusters cannot be added there (A3).
3. The cluster path cannot name a kubeconfig, and `KUBECONFIG`/`PATH` are stripped, so many real setups see an empty context list (A2).
4. kubectl add/install/key-check runs have no timeout or Cancel (A4).
5. Removing a remote pool offers "Forget it here" only for 3 of the findings, so a sign-in-refused or Docker-gone pool cannot be removed from the window (A6).
6. Container names are not unique per desktop on a shared remote engine, and the error invites removal (A7).
7. The legacy dialog skips the runner-image contract check and has no remote kind (A5, section 5).
8. Several first-start and Connect errors are swallowed (A8, A9).

---

# Parts B and C: new-chat start latency on a remote Docker pool, and the remote-session lifecycle

Scope: purlis at `main` (92d19c2320), a Docker pool on another machine reached over SSH (ADR 0078, ADR 0123).
Read-only audit. Every file:line below was read in this checkout. **Nothing here was measured.** The repo has no
tracing span or timer around any start step (`grep info_span|#[instrument]` over `runnerlinks.rs`,
`hostedsessions.rs`, `sshroute/*`, `connector.rs` finds none; `Instant::now()` there is only used for deadlines),
and `docs/spec.md`'s "Performance budgets of record" table has no row for a pool start. Numbers marked **(est.)**
are my model: docker-CLI process overhead assumed 50-100 ms, each `docker` API call 2+ RTT (version ping + call).
Constants are exact.

Paths are relative to the worktree root. `R=app/src-tauri/src/runnerlinks.rs`, `H=app/src-tauri/src/hostedsessions.rs`,
`P=crates/purlis-core/src/sshroute/pool.rs`, `F=crates/purlis-core/src/sshroute/forward.rs`,
`S=crates/purlis-core/src/sshroute.rs`, `K=crates/session-protocol/src/hostlink/keep.rs`.

---

# B. New-chat start latency on a pool

## B.0 Shape of the thing (why it is slow)

There is no persistent "connection to a runner" that a chat start reuses by default. Three layers exist:

1. **One SSH forward per pool, per process** (`ssh -T -L local.sock:remote-docker.sock host 'exec sh -c "read line…"'`,
   `S:637-672`, held in `static HELD_HERE` `F:354`). It makes the *Docker API socket* of the far machine local. It is
   the only thing that is long-lived and reused. It ends with the process holding it (`HELD_BY` pipe, `S:632`).
2. **One "link" per client link, made by running `docker exec -i <container> purlis bridge` through that forward**
   (`R:196-228` builds the `CommandConnector`; `crates/host/src/connector.rs:145` runs it). Every link, including
   every reconnect and hourly renewal, first re-runs a **check of the container** (below). The runner speaks the
   session protocol over the exec's stdin/stdout.
3. **The window's session host relays** the window's commands onto layer 2 (`crates/host/src/relay.rs:1-5`).
   "Every client link gets a link of its own to the other host, made with the client's first command"
   (`relay.rs:11`). So each project/window link pays layer 2 separately.

Start latency is therefore dominated by **layer 2's per-link check chain, which is paid more than once per start**.

## B.1 Critical path, in serial order

Legend: [S] serial and blocking, [C] cacheable/already cached, [P] parallelisable in principle.

### B1. UI to core command
`New chat` -> Tauri command -> `chats.rs` start path (local, ms). Start is cancellable
(`H:204-211` `LOOKS_FOR_A_CANCEL_EVERY`=100 ms poll, `UNTIL_THE_HOST_SAYS`=24 h). No progress per step is surfaced
to the UI: the dialog just waits. (grep of `app/src` finds only a boolean `starting` in `PlaneView.tsx:4574`.)

### B2. Route decision and host creation
`RoutedSessions::open_named` -> `on_a_pool` (`app/src-tauri/src/routedsessions.rs:297`) -> `pool_for_a_start`
(`:363`, takes the global `replacing` lock, serialises all pool starts of the project) -> `pool_of` -> `made_for`
(`:559`, `:578`) creates the `Connection` object once per pool. Cheap, local.

### B3. Profile readiness on the pool (a whole extra link) [S]  **<- hidden second link**
`chats.rs:5301`: `on_pools.ready(&project, &pool, profile)` runs **before** the harness start for any chat with a
profile. For an SSH Docker pool, `poolprofiles::yours` treats it as the person's own (`poolprofiles.rs` `yours`,
`is_remote` branch) so the full path runs (`poolprofiles.rs:99`): `yours()` -> `docker_of_named` (forward + engine
check), `tidy_once`, then `onrunner::ready_on` -> `keep_on_as` (`poolprofiles.rs:867`) -> `to_a_runner` -> `ToARunner::of_kept_waking`
-> `keep_profiles` (`R:926`) -> `on_one_link_within` (`R:683`), which builds **a brand-new `to.keep()` link**
(`runtime.block_on`, new tokio runtime per call, `R:683-715`) and waits up to `FIRST_LINK_WITHIN` for it.
That link does the whole B5-B8 chain, sends the profile, asks sign-in standing, and is dropped. Then B4-B8 happen
**again** for the relay's link. This is the single largest avoidable cost I found.

### B4. SSH forward (first start in a process only) [S, then C]
`F:363` `hold_noting` takes the global `HELD_HERE` mutex (`F:354`) **and keeps it while it opens the forward**.
`F:172` `open`:
- `PrivateDirectory::open` x2, `held.lock` take (`taken`, `F:308`, polls every 50 ms, up to `FORWARD_WITHIN`=30 s;
  `LET_GO_WITHIN`=2 s `F:302` when another process's dead holder is waited out).
- `connect::resolve` (`F:191`) = spawns `ssh -G`-style resolution (a process spawn, `RUN_WITHIN`=30 s `S:48`).
- writes pinned `known_hosts` run folder; spawn `ssh` with `common_options` (`S:591`): `ConnectTimeout=10`,
  `ServerAliveInterval=15`, `ServerAliveCountMax=3` (`S:45,55-56,614-616`), **`ControlMaster=no`, `ControlPath=none`**
  (`S:600-601`), `BatchMode=yes`. So **every forward is a full TCP + SSH key exchange + auth** (est. 4-6 RTT + key
  signing; more if the key is in an agent that prompts or the remote login shell is slow, since the remote command
  `exec sh -c '…'` goes through the user's login shell).
- `wait_for(socket, FORWARD_WITHIN=30 s)` polls every **50 ms** (`F:241`, loop at ~`F:276-290`). The listener appears
  as soon as ssh binds locally, so this is usually short; a 50 ms poll granularity adds <=50 ms.
- A watch thread polls `try_wait` every **200 ms** (`F:109-125`) for the life of the forward (idle wake-ups, no latency).
After this the forward is reused by every later `docker` run (each run is just a new SSH channel, cheap). It is lost
whenever the owning process exits, and reopened by the next start. ADR 0123/2026-11-17 explicitly rejected
`ControlMaster`/`ControlPersist` (`docs/adr/0123/2026-11-17-a-forwards-lock-never-outlives-its-ssh.md`) because the
forward must end with its owner and ssh multiplexing still needs the same `-L`.

### B5. Engine pin check through the forward [S, cached 30 s]
`P:126` `docker_through`: `P:144` consults the failing-forward memory (see B.4 dead time), `hold_noting`, `pointed`,
then `if !opened && checked_lately(...)` returns (`P:170`, `ENGINE_CHECKED_FOR`=30 s `P:370`). Otherwise
`connect::engine_holds` = `docker info --format {{json .}}` (`dockerpool.rs:4056`, the *full* info JSON, parsed for the
engine id): 1 docker run. A just-opened forward is always re-checked. A forward held from before whose engine is silent
is closed and reopened once (`checked_or_opened_again`, `P:256`, wait for ssh to notice up to `NOTICED_WITHIN`=2 s
`P:224`, polling 50 ms `P:236`) (this is #7116).

### B6. Container check before every connection [S, NOT cached]  **<- biggest per-link cost**
`R:205-208` (inside the connector's `before_each`, `R:200-221`): `docker_of_named` (B5), then
`checked_or_woken(..., wakes = true)` (`R:112`) -> `Docker::resumed` (`dockerpool.rs:4140`):
1. `check_daemon` = `docker info --format {{json .SecurityOptions}}` (`:3214`),
2. `docker container inspect` (`:4144`),
3. container running -> `self.check(made)` -> `checked` (`:3880`) which repeats `check_daemon` (info again),
   `container inspect` again, `docker volume inspect`, and (for the runner seccomp profile) a **`docker exec`
   `_no-namespaces -- /bin/sh -c "exit 0"`** (`:3925`).
That is **6 further serial docker CLI runs** (+1 from B5 when not cached) = **up to 7 serial `docker` processes,
each over the SSH forward, each preceded by `prepared()` (a checked copy of Docker's settings written to disk,
`dockerpool.rs:2846`, `connector.rs:159`)**, before the one `docker exec -i … purlis bridge` that is the actual link.
`info` and `inspect` are each done twice (redundant: `resumed` reads, then `check` re-reads).
Estimate **(est.)**: 7 runs x (2 RTT + 50-100 ms) -> ~1 s at 30 ms RTT, ~3 s at 150 ms RTT, per link.
Each run is held to `READS_WITHIN`=20 s (`dockerpool.rs:2657`); the whole check+prepare to `CHECK_WITHIN`=60 s (`R:228`,
`R:1383`). If the runner container is stopped (host reboot, #7116; or self-suspended after idle, #2051) `resumed`
also does `docker start` + a fresh full `check` (7 more runs).

### B7. `docker exec -i … purlis bridge`, runner process start [S]
`connector.rs:168+` spawns `docker exec`. API: exec create + start (2+ RTT) through the forward, then the `purlis bridge`
process (`crates/purlis-cli/src/bridge.rs`) connects to the runner host's link socket, polling every **200 ms** up to
`HOST_WITHIN`=60 s (`bridge.rs:28,166`) for a runner host that is still starting (relevant right after a `docker start`).

### B8. Link handshake [S]
`connect_kept` (hello, encrypted handshake, session proof): each held to `HANDSHAKE_TIMEOUT`=10 s
(`crates/session-protocol/src/version.rs:48`). Normal case ~3 RTT (est.). The desktop's first wait for it is
`FIRST_LINK_WITHIN` = `CHECK_WITHIN(60) + 2*HANDSHAKE(10) + 10` = **90 s** (`R:1390`).

### B9. Relay carry-over and `HoldProject` [S]
The relay's `keep` (`crates/host/src/relay.rs:586`) runs `carry_over` on each new link (re-holds projects, resubscribes,
reattaches views). Then `held_on` (`H:3901`) sends `HoldProject` (`H:3988`), `DealtAlready`, `Board`, `ProgramsOf` (`H:4044`)
**as sequential calls on the link**: >=3-4 RTT (est.), each `call_of_a_start` (`H:1974`).
First-link wait for a start is `LINK_WITHIN`=30 s (non-cancellable, `H:147`) or 24 h (cancellable, `H:3921`).

### B10. Code sync (git bundle) [S]
`H:3609` `sends_the_code` (via `link_of_a_start`, `H:3600`) -> `runnercode::send` for the project's own repo, once per
link generation (`on.code_sent`); then `H:5009` `send_workspace` -> `runnercode.rs:537` loops **each clone of the
workspace serially**, each = local `git bundle` into a temp dir (`runnercode.rs:~80`), `link.send_code`, checkout on the
runner. Incremental after the first (the `Sent` map of what the runner holds), but `forget(sent)` at `H:3966` throws it all
away whenever the project is *held again on the same link* (which a link drop + `link_is_down` triggers, `H:3935-3940`),
so the **next start after any reconnect re-sends whole repositories**. Cost scales with repo size and uplink; no
parallelism across clones. Bound by the pool's `answerWait` (default 30 s, max 600 s; `poollimits.rs:344`).

### B11. `StartProgram` on the runner: harness spawn [S]
`H:4987` `open_named` -> `started` (`H:4641`) -> `call_start` (`H:2062`) -> relay -> runner's `hosting.rs:4214`
`Command::StartProgram`: compose sandbox plan, spawn the harness PTY. Waits for the answer with a 100 ms cancel poll
(`LOOKS_FOR_A_CANCEL_EVERY`), and for non-cancellable starts the pool's `answerWait`=30 s default
(`H:2066-2069`, `:2118-2128`).

### B12. First prompt usable
The window then attaches a view (snapshot over the link), and the harness itself must boot/authenticate (credentials
were kept on the runner by B3; no per-start sign-in unless signed out, in which case start refuses and raises an Ask,
`onrunner.rs:1114-1130`). Harness cold start is outside purlis's control.

### Image pull
There is **no image pull on the chat-start path.** The runner image is pulled/made when the pool is added or the runner
made ("Make the runner", `Docker::run` in `dockerpool.rs`; grep for `"pull"`/`"load"` in `dockerpool.rs` finds none on the
start/link path). A stopped runner is `docker start`ed, not recreated (ADR 0078/2026-11-17). First-time pull cost is
paid once at pool setup. Runner image version drift is only logged (`R:~430` `behind`).

## B.2 Total (est.), warm vs cold

| scenario | serial steps | est. latency (RTT 40 ms / 150 ms) |
|---|---|---|
| warm: forward held, engine checked <30 s ago, runner running, profile on the chat | B3 link (6 docker + exec + handshake + keep) + B6/B7/B8 for the relay link (6 docker + exec + handshake) + B9 + B10 + B11 | ~3-4 s / ~8-12 s |
| warm, no profile on the chat (skips B3) | halves the link work | ~1.5-2 s / ~4-6 s |
| cold: first start after app launch (forward not held) | + B4 (ssh connect/auth) + B5 `docker info` | +0.5-1 s / +1.5-3 s |
| after remote reboot (runner exited) | + `docker start` + 2nd full check + `purlis bridge` wait for runner host (<=60 s poll) | +5-15 s typical, more if kernel/docker still coming up |
| after a reconnect, then a start | + whole-repo re-send (B10) | depends on repo size |

These are model figures to rank the steps, not measurements. The relative ordering (B3+B6 double-paid >> forward >>
handshake) follows from the code.

## B.3 What can be parallel, cached, or kept warm

| Opportunity | Where | Notes |
|---|---|---|
| **Drop the duplicate link in B3**: do the profile keep on the relay's own link (one link, then `keep_profile` over it), or cache the "profile kept for this runner" result for N minutes | `chats.rs:5301`, `poolprofiles.rs:99,867`, `R:926` | halves per-start link cost. `ready()` already caches nothing; `tidy_once` is once-only |
| **Cache the container check** like the engine check (`ENGINE_CHECKED_FOR`=30 s, `P:370`) | `R:208`, `dockerpool.rs:4140` | the container's "as made" facts do not change in seconds; only "is running" does. The check's `info`+`inspect` are done twice inside one call |
| **Collapse the 6-7 docker runs into 1-2**: one `docker container inspect` + `docker info` (already `{{json .}}` in `engine_id_said`, so `SecurityOptions` is in that same JSON) | `dockerpool.rs:3214,3880,4056,4140` | engine id (B5) and daemon seccomp check (B6) both read `docker info`: read it once. `resumed` then `check` re-reads info+inspect |
| **Run the checks in parallel** (info, inspect, volume inspect are independent reads over the same forward; each is an SSH channel) | `dockerpool.rs:3880` | forward already multiplexes channels. ~7 serial -> ~2 rounds |
| **Pre-warm the forward** at app start / when a workspace on a pool opens (open it and run B5 in the background) | `F:363`, `routedsessions.rs:297` | removes B4+B5 from the first start. The forward is process-held, so it is lost on every app/host restart |
| **Pre-warm the link** (keep the relay link up while a workspace on a pool is open, rather than on first command) | `relay.rs:586` | the link is lazily created "with the client's first command". Hourly renewal `renew_after` (`hostlink.rs:239`) already keeps it fresh |
| **Keep a warm runner** | n/a | runner is one long-lived container per pool already; only post-reboot/post-suspend wake is cold. Auto-start on app launch when a pool is "on" would hide it |
| **Overlap code sync with link setup** | `H:3600`, `runnercode.rs:537` | B10 could start bundling locally (git bundle) while the link/check completes; clones could send concurrently |
| **Don't resend whole repos after a re-link** | `H:3966` (`forget(sent)`) | ask the runner what it holds (`ReposHeld`, already exists, `H:3859`) instead of assuming nothing |
| **ControlMaster/ControlPersist for the forward** | `S:600-601` | rejected in ADR 0123/2026-11-17 for lifecycle reasons; a master per pool owned by the same process would still avoid reconnect cost only if the process survives, which it does not across restarts. Not a quick win |

## B.4 Retries, backoffs and dead time

| What | Constant | Effect |
|---|---|---|
| Link retry pause after a failed dial | `FIRST_PAUSE`=250 ms doubling to `MOST_PAUSE`=30 s (`K:125,128,~205-215`) | sequence 0.25, 0.5, 1, 2, 4, 8, 16, 30, 30... Each failed dial can itself take up to `CHECK_WITHIN`=60 s |
| **Failing-forward memory (a second, longer backoff)** | `RETRY_FIRST`=5 s, doubling, `RETRY_MOST`=120 s (`P:524,527`); `FINDING_KEPT_FOR`=300 s (`P:531`) | After a forward fails to open, `docker_through` returns the **remembered finding without contacting the machine** until `next_try` (`P:144-150`: `Ask::Answer`). Runs 5, 10, 20, 40, 80, 120 s. So when the far machine comes back, recovery can lag by up to ~120 s (+ up to one 30 s link pause) even though link retries run every <=30 s |
| **Try now does not clear that memory** | `keep.rs:118` `try_now`, `relay.rs:502` `link_now` | `try_now` only wakes the link pause (`K` `woken.notified()`); the dial then reaches `docker_through`, which answers the cached finding if `now < next_try`. Only `look_again()` (`P:431`) clears it, and it is called only from Pool setup's **Check again** (`poolsetuptab.rs:393-397`), never from `runner_link_now` (`reconnecting.rs:90`) or the Notice's **Try now**. Verified: the only non-test caller of `look_again` is `poolsetuptab.rs:397`. The Notice says "trying at most 30 seconds apart" (`RunnerLinkDownNotice.tsx` `downSaid`), which the 120 s memory contradicts |
| Window Notice grace | `QUIET_FOR_MS`=30 s, read-again 15 s (`RunnerLinkDownNotice.tsx:16,20`) | UI only |
| Start while the relay link is `Down` | `relay.rs:553` `not_linked`, `relay.rs:512-` `up()` | for a **Docker** pool a start during `Down` is **refused at once** ("Runner not linked … send this again once the runner is back"), because `RunsAgain` (`routing.rs:208`) only knows cluster pools, so `waits_for_the_runner` (`routing.rs:234`) returns false. Only `Linking` waits. So a start 1 s before the link recovers fails and the user must retry |
| Forward dead holder wait | `LET_GO_WITHIN`=2 s, poll 50 ms (`F:302,337`); `NOTICED_WITHIN`=2 s (`P:224`) | dead time, but only on the recovery path |
| Forward-open waits | `FORWARD_WITHIN`=30 s (`S:51`), `ConnectTimeout`=10 s | a down host costs 10 s per attempt before failing, plus `diagnosed` (resolve, `ANSWER_WITHIN`=5 s TCP reach `connect.rs:259`, a probe up to `RUN_WITHIN`=30 s) at the first failure and every `FINDING_KEPT_FOR`=300 s |
| Keep-alive detection of a lost forward | 15 s x 3 = 45 s (`S:55-56`) | a silently dead network takes up to ~45 s to be noticed by ssh, up to ~30 s by the link's own ping (`hostlink.rs:237-239`: ping 10 s, dead after 30 s silence) |
| Hourly renewal | `renew_after`=1 h (`hostlink.rs:239`) | each renewal re-runs the full B6 chain (7 docker runs) in the background; benign but constant load |
| Global lock held across the forward open | `F:363-378` `HELD_HERE` | a slow open for pool A blocks `dropped`/`hold` for **every** pool for up to 30 s (+2 s) |

---

# C. Lifecycle state machine of a remote session

## C.0 There are four layered state machines, not one

1. **Forward** (per pool, per process): `F` `Held::{Own,Shared}` + `Lease`.
2. **Link / relay standing** (per client link): `crates/host/src/relay.rs:430` `enum Standing { Linking, Up, Down(Down), Over(Option<Down>) }`,
   driven by `relay.rs:586` `keep` over `K:133` `keep_linked`.
3. **Window's view of it** (per project connection): `H:1609` `enum Reconnect { Started, Over, Drawn, KeyChanged, Gone }`
   -> `H:3796` `reconnects_of` -> `app/src-tauri/src/reconnecting.rs:19` `Reconnection { Reconnecting, Reconnected, KeyChanged, RunnerGone }`
   + `LinkDown { why, since }` (`reconnecting.rs:49`) -> UI `Reconnecting…` per chat and `RunnerLinkDownNotice` per pool.
4. **Chat/run** state: the plane's `RunState` (ADR 0076), plus "left"/"stayed"/"lost" bookkeeping in `routedsessions.rs` and `hostlost.rs`.
Plus the **pool record**: kept / draining / removed / forgotten (`dockerpools.rs:552,630,655`).

## C.1 State/transition table

| # | State | Where it lives | Entered by | Leaves by | UI shows |
|---|---|---|---|---|---|
| 1 | **No forward** | `F:354` `HELD_HERE` has no entry | app/host start; `let_go`/`let_go_of_all` (`F:~410,~425`); pool removed | a start/link calls `hold_noting` (`F:363`) | nothing |
| 2 | **Forward opening** | `F:172` `open`, `wait_for` `F:241` | `hold_noting` | socket answers -> 3; ssh ended / 30 s -> failure -> 14 | start spinner |
| 3 | **Forward up** (`Own` or `Shared`) | `F` `Forward` + `watch` thread `F:114` | `open` ok | ssh ends -> watch lets go within 200 ms -> 1; `dropped()` `F:397` true -> reopened by next `hold_noting` | nothing |
| 4 | **Forward silent** (looks up, engine doesn't answer; after remote reboot) | `P:256` `checked_or_opened_again` | engine check fails `EngineSilent` on a held forward | `dropped_within` 2 s then reopen -> 2; or kept as "engine slow" | start slow, then ok or error |
| 5 | **Link Linking** (first link or after `Over` re-ask) | `relay.rs:430` `Standing::Linking`, `relay.rs:~600` | first client command; `again.notified()` (`relay.rs:~680`) | `Linked::Up` -> 6; `Linked::Down` -> 8 | start waits |
| 6 | **Link Up** | `Standing::Up(upstream)` | `carry_over` ok (`relay.rs:~612-625`) | link ended / ping dead (30 s) -> 8; renewal at 1 h -> 6 (new, old retires) | chat live |
| 7 | **Starting a chat** | `H:4641` `started`, `H:3901` `held_on` | person's New chat | `ProgramStarted` -> running; refusal/cancel -> error | dialog |
| 8 | **Link Down / reconnecting** | `Standing::Down(down)`, `LinkTold::Down` (`relay.rs:~690`), `Pushed::LinkDown` `H:3340` | dial fails (docker check fail, ssh fail, handshake fail, ping dead, container removed) | `Linked::Up` -> 6 (+`link_back`); `key_changed` -> 10; `gone` -> 11; `link_now` wakes pause | chat dimmed `Reconnecting…`; after 30 s Notice with why + Try now + Check pool |
| 9 | **Link renewing** | `K` `hold` renewal branch | `renew_after` 1 h | new up, old retired when reader `moved()` or `deadline` | nothing |
| 10 | **Key changed** | `Down.key_changed`, `Reconnect::KeyChanged` | runner answers other key / operator key gone | person removes + re-adds pool | Notice `RunnerKeyChangedNotice` |
| 11 | **Runner gone** (container removed outside purlis) | `Down.gone`, `relay.rs` `Standing::Over(Some(gone))`, `FarEndGone` | `Docker::missing` says removed (`R:215-220`) | person "Make the runner again" | `RunnerGoneNotice`; chats `RunnerGone` exit (`H:340,587-610`) |
| 12 | **Over** (kept link ended, nothing more to try) | `Standing::Over` | `no_more` from `keep_linked` | next command re-asks (`relay.rs:~519-525`, once) | refused "Runner not linked" |
| 13 | **Runner stopped** (reboot / suspended) | container `Exited` | machine reboot; runner self-suspends after idle (#2051) | next link's `resumed` does `docker start` (`dockerpool.rs:4140`) -> 6 | just a slower start |
| 14 | **Forward failing (backed off)** | `P` `FAILING`, `Failing{next_try}` | `forward::open` Err (`P:144-165`) | `ask()` after `next_try`, `look_again` | link Down with Pool-setup words |
| 15 | **Window restarted** | `routedsessions.rs:463` `settle_what_was_kept`, `ChatsKept`, relay `name_window`/`take_back` (`relay.rs:~60-75`) | app/host restart | fresh link (all of B) then `HoldProject`+`ProgramsOf` (`H:4044`) -> chats reattached (`routedsessions.rs:1535`) | tabs come back as `Reconnecting…` then live |
| 16 | **Pool draining** | `routedsessions.rs:363` `pool_for_a_start`, `stayed` map | runner replaced after update | chats end; start goes to new runner (`A_DRAINING_RUNNER_STARTS_NONE`) | chats stay on old runner |
| 17 | **Pool forgotten / removed** | `dockerpools.rs:630,655,666`; `H:564` `pool_removed`, `H:672` `links_to_pool_end` | Remove, or Forget it here (#7122) when machine silent | terminal | links end; chats `RunnerGone`/"ended with that runner"; workspaces run chats locally again |
| 18 | **Host process crashed** | `hostlost.rs`, `H:2848` `everything_ended_with_the_host` | local session host exits | resume per ADR 0076 table | `Lost` notice + Resume |

## C.2 ASCII diagram (link layer, the one the person feels)

```
                       first client command
   [no forward] ───────────────┐
        ▲  ssh ends            ▼
        │ (watch 200ms)   [forward opening] ──fail──► [forward FAILING: 5,10,20,40,80,120 s memory]
        │                      │ ok                          ▲ look_again only (Check again)
   [forward up] ◄──────────────┘                             │ next_try elapsed
        │ docker check chain (B5/B6) ×7 serial
        ▼
  ┌──────────┐ dial ok + carry_over ┌──────────┐
  │ Linking  │─────────────────────►│   Up     │◄─── renew every 1 h (Up→Up)
  └──────────┘                      └────┬─────┘
       │ dial fails                       │ link ended / ping silence 30 s / container gone
       ▼                                  ▼
  ┌──────────┐◄───────── pause 250ms…30 s, doubling; Try now skips pause ─────────┐
  │  Down    │──── dial ok ───────────────────────────────────────────────────────┘ → Up (+link_back)
  └──┬───┬───┘
     │   └─ key_changed ──► [KeyChanged] (terminal until pool re-added)
     └──── gone ──────────► [RunnerGone] (terminal until runner remade)
  start while Down (Docker pool): refused immediately.   start while Linking: waits.

  UI:  Up→Down: panes "Reconnecting…" at once;  Down >30 s: Notice (why, Try now, Check pool)
       Down→Up: link_back → "Reconnected"; 15 s poll clears Notice for pools with no chats
```

## C.3 Where each requested state lives

- **starting**: `H:4641` `started`, `H:3901` `held_on`; non-linked `Standing::Linking` `relay.rs:430`.
- **running**: `H` `lock(&self.running)` map; `routedsessions.rs` `shared.chats` (chat number -> pool), `shared.stayed`, `shared.left`.
- **link down / reconnecting**: `relay.rs:586` `keep` + `LinkTold` (`relay.rs:~690-730`) -> `Pushed::LinkDown` handler `H:3340-3358`
  (`lost_its_host`, `runner_log.link(true)`, `reconnect(Started)`) -> `reconnects_of` `H:3796` -> `reconnecting.rs` events. `since` is kept
  across failed retries (`H:3801-3807`).
- **restored after app restart**: `name_window` digest on the link so the runner gives chats back (`relay.rs` header "A window that comes back takes back its runner chats too", test `crates/host/tests/it/a_relay_links_again_to_the_other_host_on_its_own.rs:840`; app side `H` tests `:3345,:3663`); `ChatsKept` + `settle_what_was_kept` `routedsessions.rs:463`; `reattach` `routedsessions.rs:1535`; `ProgramsOf` read at `H:4044`.
- **host reboot**: container `Exited(0)` has no restart policy by design (ADR 0078/2026-11-17 item 1); next chat's link `resumed` starts it (`dockerpool.rs:4140`); the forward is first reopened once (`P:256`); first-link wait outlasts the check (`R:1390`).
- **pool forgotten**: `dockerpools.rs:630` `forget_docker_pool`, `:655` `forgotten_here`, `:666` `let_go_of`; ends links via `H:564,672`.

## C.4 Untested or ambiguous transitions (checked by grepping tests)

1. **Try now vs. failing-forward memory** (C: Down -> Linking while `FAILING.next_try` is in the future). Tests: `crates/host/tests/it/a_relay_links_again_to_the_other_host_on_its_own.rs:475` (`link_now` ends the pause) uses a stand-in cable, and `H` tests `:3328` only check the command is sent. `P` test `:630` shows an ask inside the wait is answered from memory. **No test exercises `link_now` -> `docker_through` and no code clears `FAILING` on Try now.** Ambiguous/likely bug (B.4). Also `FIRST_LINK_WITHIN` is untested end to end.
2. **Reboot recovery path as a whole** (#7116): unit tests cover the pieces (`P` tests `:767,796,809`; `ADR` item 2), and `crates/purlis-core/tests/it/a_remote_docker_pool_is_reached_over_ssh.rs:259` `a_killed_connection_is_noticed_and_opened_again` kills ssh. **No test reboots a container-stopped + forward-silent combination through `resumed` -> `bridge` wait -> handshake.** The live finding was manual.
3. **Start during `Down` on a Docker pool is refused rather than waiting.** `routing.rs:208-248` and tests in `crates/host/src/routing/tests.rs` cover the cluster (`RunsAgain`) wait; I found no test pinning "Docker pool, link Down, start -> immediate refusal" as intended behaviour. It is the documented design for non-cluster pools (#4403 comments) but the person experiences it as flaky starts right after an outage.
4. **Whole-repo re-send after reconnect** (`H:3966`): `forget(sent)` is exercised in `runnercode` tests but no test asserts the cost/behaviour after `link_is_down` in `held_on`.
5. **Duplicate link at B3**: `poolprofiles.rs:99` ready path is tested for outcomes (`onrunner` tests), not for "how many links a start makes".
6. **Over state**: `Standing::Over(None)` then re-ask once (`relay.rs:~519`); `a_relay_links_again…` covers key changed and renewal, not a Docker `Over` for a non-gone reason (`no_more` happens when the receiver is dropped or the kept link ended; `gone` is the only producer in practice).
7. **App restart mid-outage**: window restarts while the link is Down. `H:3663` tests the quick-restart naming race; I found no test where the restart lands during a Down with chats expected to come back dimmed then live. `settle_what_was_kept` explicitly leaves records "for the next window" when the pool doesn't answer (`routedsessions.rs:463-475`), which means a chat can stay pending across multiple restarts. Ambiguous end state.
8. **Forward owned by another process** (`Held::Shared`): `forward/tests.rs:90,139` cover dead holder and answer-only use. The `HELD_HERE` global mutex held across a 30 s open (`F:363`) has no test about cross-pool blocking.
9. **Forget it here with live chats**: `dockerpools/tests.rs:513,549` cover the offer/forget bookkeeping. The chat-side outcome (`pool_removed` -> `runner_went` -> `Exit::Signal(RunnerGone)`, `H:564-575,587-610`) is covered for Remove (#4350) but I did not find a test that drives `forget_docker_pool` through to a running chat's tab state. The notice wording for chats on a forgotten pool is not distinguished from "Runner gone".
10. **Link Up while the *forward* is silently dead** (no 45 s ssh keep-alive hit yet): the link's own ping (30 s) and ssh's (45 s) race; the window says "Reconnecting…" only after the link layer notices. Up to 30-45 s of an apparently live chat that does not respond. Untested at the app level (ssh-level covered by `sshroute/tests.rs:155`).

## C.5 Recent fixes (git log, `main`)

- **#7116 (7ef6d5dcb8)** "A remote Docker pool comes back after its machine restarts" (live finding on Ubuntu 24.04 over SSH after a kernel update).
  Three changes: (1) `FIRST_LINK_WITHIN` (90 s) now outlasts the connector's `CHECK_WITHIN` (60 s) + both protocol deadlines, so the link's own reason arrives first; the runner calls a stream closed mid-handshake `LinkRefused::WentAway`, not "not the host this link was paired with"; `CommandConnector::within` holds the check to a deadline (`connector.rs:106`). A down link is said with "Check pool <pool> in Pool setup" (`R:with_a_way_on`). (2) `checked_or_opened_again` (`P:256`): a forward held from before that goes silent is dropped and reopened **once** before the check refuses (ADR 0123 amendment 2026-11-17). (3) a stopped runner is started by the next chat: `of_kept_waking` + `Docker::resumed`; deliberately **no Docker restart policy** (ADR 0078 amendment 2026-11-17). Build on: the three constants and the one-retry rule.
- **#7122 (7105c61f9b)** "A pool whose machine doesn't answer can be forgotten here". New `PoolRemoval` / `forget_docker_pool` (`dockerpools.rs:552,630`) with a `pool-unreachable:<pool>` Notice and **Forget it here**: contacts nothing, forgets the pool and its runners, workspaces fall back to running chats locally, window links end, tells the person the `docker rm`/`volume rm` to run there. Only offered when the machine does not resolve/answer, never when it answers and refuses. Build on: the same "terminal state with a named way out" pattern would suit a runner-link Down that is hopeless.
- **#7130 (834935a196)** "A forward whose ssh ended lets go of its lock". Found live: the window held `held.lock`, its ssh had ended, the socket file stayed, and a second process (the session host) ran `docker info` against a dead socket for hours. Fix (`F:114` `watch`, `F:308` `taken`, `F:397` `dropped`): the holder lets go of socket then lock within 200 ms of ssh ending; another process uses a held forward only while its socket **answers a connect**; a lock held over a dead socket is waited out 2 s (`LET_GO_WITHIN`) then refused with "quit that process". ADR 0123 records why not Docker's `ssh://` transport and why not `ControlMaster`. Build on: `answers(socket)` as the liveness primitive (`F:~345`), reusable for B3-style pre-checks.
- **#7135 (cf302a779e)** "A runner chat's Reconnecting… says why past 30 seconds, with Try now and Check pool" (ADR 0078/2026-11-18). New `RunnerLinkDownNotice` (`QUIET_FOR_MS`=30 s), session protocol 1.92 `link_now` (relay ends the pause, `relay.rs:502`), `link_down` re-sent when the reason changes (`LinkTold::down`, `relay.rs:~700`), core keeps last reason per pool (`runner_links_down`, `reconnecting.rs:67`). Build on: all the surface exists; the gap is that **Try now cannot actually bypass `FAILING`** (C.4 #1) and that a start in `Down` still fails instead of waiting.

---

# Top problems found

1. **A start on a pool makes two (or more) full links, each preceded by ~6-7 serial `docker` runs.** The profile-readiness link (`chats.rs:5301` -> `poolprofiles.rs:867` -> `R:926`) is thrown away, then the relay makes its own. Highest-value fix: reuse one link, or cache the check. (B3, B6)
2. **The per-connection container check is not cached and is redundant**: `docker info` x3 and `container inspect` x2 per link, serial, each with settings copy, over SSH (`dockerpool.rs:3214,3880,4056,4140`). The engine check has a 30 s cache (`P:370`); the container check has none. Reads are independent and could be parallel or one call. (B5, B6)
3. **Try now cannot beat the failing-forward backoff** (5 s doubling to 120 s, `P:524-531`), and the Notice promises "at most 30 seconds apart" while real recovery can lag ~2 minutes. `runner_link_now` never calls `look_again` (only Check again does, `poolsetuptab.rs:397`). No test covers it. (B.4, C.4 #1)
4. **A start while the link is `Down` on a Docker pool is refused instantly** (`routing.rs:208-248`: `RunsAgain` is cluster-only), even when the next dial is <=250 ms-30 s away. After a blip the person gets "Runner not linked … send this again". Waiting for `Linking`/short `Down` (as is done for cluster pools) would hide most blips.
5. **Whole-repository re-send after any reconnect** (`H:3966` `forget(sent)`; `runnercode::send` is whole when `Sent` is empty), serial per clone (`runnercode.rs:537`). This makes the first start after an outage the slowest. The runner can already report what it holds (`ReposHeld`).
6. **Cold forward costs a full SSH handshake every app/host launch** (`ControlMaster=no`, `S:600`; forward ends with its owner, `S:632`) and is opened lazily inside the first start while holding the global `HELD_HERE` mutex for up to 30 s (`F:363`), blocking other pools' `hold`/`dropped`. No pre-warm on workspace open.
7. **No instrumentation**: not one tracing span or elapsed-time log around forward open, docker check runs, link dial, hold, code send or start. The estimates above cannot be replaced by data until each step logs its duration. A `purlis start-profile` style timing, or `tracing::info_span!` per B-step, is the prerequisite for any tuning.
8. **Reboot recovery depends on ordered luck**: forward-silent -> reopen once (2 s notice window) -> `docker start` -> second full check -> `purlis bridge` polling 200 ms up to 60 s -> handshake, with no end-to-end test (C.4 #2) and a 90 s overall first wait that is longer than any other timeout the person sees.
9. **Lifecycle UX gaps**: no per-step start progress (B1); the pool-forgotten and runner-gone terminal states are not distinguished for the chats on them (C.4 #9); restart-during-outage leaves chats "pending" across windows (C.4 #7).

No undisclosed security issue was found in the paths read (forward lock/socket handling is same-user `PrivateDirectory`, pinned `known_hosts`, `BatchMode`, `ForwardAgent=no`; all consistent with ADR 0123), so there is no `part-BC-SECURITY.md`.

---

# Parts D and E: tests today, and observability today (remote path)

Read-only audit of the worktree. Paths are relative to the repo root. Counts are `#[test]`/`it(` greps (fn counts differ where helpers exist). No network, no repo edits.

## D. Tests today

### D.0 Headline

A real-infrastructure harness for the SSH and Docker pool path already exists and runs in CI. The proposal below is therefore a small extension of `tests/remote-docker/`, `tests/ssh-runner/` and the `remote-parity` job. It is not a new harness. What is missing is fault injection on a live link: nothing in CI drops a link or restarts an sshd while a chat is running on a pool, or makes a start slow or an image pull fail.

### D.1 Rust, `purlis-core` integration tests (`crates/purlis-core/tests/it/`)

All of these are real-infra: they need a fixture from an `up.sh` script. With no fixture the test returns early and passes, because `fixture()` returns `None`. See `a_remote_docker_pool_is_reached_over_ssh.rs:12-17`, `:43-47` and `an_ssh_runner_host_outlives_the_sign_out_and_a_restart.rs:78`. That early return is a silent-skip risk; see Top problems.

| File | #tests | Covers |
|---|---|---|
| `a_remote_docker_pool_is_reached_over_ssh.rs` | 14 | Real sshd + dind. Host key met, pinned and trusted. Rotated host key refused, also with the person's own known_hosts. Changed engine id refused before anything runs. `a_killed_connection_is_noticed_and_opened_again`. Account not in the docker group. Unresolvable name. Forward lifecycle: `the_forward_ends_with_the_process_holding_it_even_with_a_docker_exec_open`, `every_forward_this_process_holds_ends_as_it_lets_go_of_all`. Person's ssh settings never ride the forward. sshd refusing forwarding. Re-trust after a host key change or an engine change. Settings folder with a space in its path. |
| `a_remote_docker_pool_is_reached_over_tls.rs` | 6 | dind's own CA. CA file no longer matching its pinned fingerprint. A CA that did not sign dind's cert. Plain TCP refused. A vault-held key left on no disk. CA re-trust. |
| `a_remote_docker_pool_is_reached_through_a_docker_context.rs` | 5 | `docker context` ssh and tls contexts read into a route. Context drift changes nothing a pool runs on. Context removed. |
| `a_remote_docker_pool_doctor_names_each_fault.rs` | 14 | Every doctor finding is produced from a real fault: wrong_name, port_closed, no_agent_key, no_socket, no_docker_group, plain_tcp, bad_tls, unsupported_architecture (a riscv64-only image), full_disk (a fake `df` image), image_unreachable, context_drift, engine_id_changed. Also the green case with a measured round trip. |
| `a_remote_podman_pool_is_reached_over_ssh.rs` | 3 | Rootless Podman behind sshd: pin by socket plus graph root, checked as rootless Podman writes it, graph-root change is a pin change. |
| `an_ssh_runner_takes_a_new_version_beside_the_old.rs` | 1 (556 lines) | Ubuntu sshd with no Docker. Install of the signed purlis over ssh. A digest mismatch is refused. Version 1 with five chats, then version 2 beside it. |
| `an_ssh_runner_host_outlives_the_sign_out_and_a_restart.rs` | 1 (667 lines) | systemd container behind sshd. `purlis serve --install`. Every SSH connection cut and the account signed out, with the host and chats surviving. `docker restart` of the whole machine, then the host returns and settles from its record. Old-version host self-stops. |
| `a_docker_pool_runner_is_linked_through_docker_exec.rs`, `a_killed_docker_exec_closes_what_it_ran_on_a_runner.rs`, `code_crosses_to_a_docker_pools_runner_and_back.rs`, `a_chat_moves_to_a_docker_pools_runner_and_back.rs` | 1 each | Local Docker daemon: link over `docker exec`, kill of the exec, code (git bundle) transfer, chat move. |
| `a_pool_container_is_checked_on_this_host.rs` | 1 | `dockerpool::check` against Docker/AppArmor and Podman rootful/rootless, with recorded host outputs in `crates/purlis-core/src/dockerpool/hosts/`. |
| `a_chat_runs_on_a_cluster_runner_through_kubectl_exec.rs` | 4, all `#[ignore]` (`:256`) | Real kind cluster plus operator, run with `--ignored` by `operator.yml`. |
| `a_chat_mints_on_a_cluster_runner_with_its_desktop_away.rs`, `pool_setup_on_a_kind_cluster.rs` | 1 each, `#[ignore]` | The same, via kind. |
| `a_runners_turns_come_back_with_its_code.rs` | 11 | Runner turns and code (`tests/it/*`, not real infra by name). |

Unit tests inside `crates/purlis-core/src`, by `#[test]` grep (approximate, includes nested test modules): `sshroute` 56, `tlsroute` 23, `dockerpool` 179, `dockercontext` 15, `clusterpool` 56, `pools` 183, `poolsetup` 251, `doctor` 362 (the whole preflight doctor, not only pools), `runnerlink` 35, `runnerhost` 31, `runnerinstall` 54, `runners` 73, `runnerimage` 7, `codelink` 140, `linkkey` 4.

### D.2 Rust, other crates

- `crates/session-protocol/tests/it/` has 40 files. They test the link layer over real sockets, pipes and a child's stdio: `a_link_keeps_itself_up.rs`, `a_view_is_attached_again_by_epoch_with_backoff.rs`, `a_peer_that_stalls_the_handshake_is_refused_in_time.rs`, `a_dialer_that_goes_away_mid_handshake_is_not_called_another_host.rs`, `a_client_killed_outright_loses_its_views_and_the_next_one_reattaches_over_a_real_socket.rs`, `the_session_protocol_runs_through_a_link_between_two_hosts.rs`. The transport is in-process or loopback; no ssh and no netem.
- `crates/host/tests/it/` has 22 files, including `a_relay_links_again_to_the_other_host_on_its_own.rs`, `a_runner_left_without_a_link_suspends_itself.rs`, `a_window_that_comes_back_takes_its_programs_back.rs`, `a_window_takes_back_its_own_runner_chats_sandboxes.rs` and `the_kill_switch_reaches_every_runner_this_machine_keeps.rs`. `crates/host/src/routing/tests.rs` has 21 tests.
- `crates/purlis-cli/tests/it/` has `pool_setup.rs`, `a_pools_limits_at_the_command_line.rs`, `a_profile_*_on_a_pool_*.rs`, `runners_left_behind_at_the_command_line.rs`, `purlis_serve_*` (event log, SIGTERM as quit, hooks, own proxy confinement). These run a real `purlis` binary locally. There is no ssh.
- `crates/cluster-doctor/src/lib.rs` has 520 lines and no `#[test]`. The tests are elsewhere or in the operator workspace (not verified).

### D.3 App Rust (`app/src-tauri/src`)

All in-process with fake hosts, apart from the `behaviour` suite's pool targets.
- `routedsessions/tests.rs`: 52 tests, 1582 lines. Plus `routedsessions/chatskept.rs`, which holds the store of which pool a chat ran on.
- `sessionhost/tests.rs`: 24. `sessionhost/breaker.rs`: 4. The breaker is 3 exits within a window, then the "failing" Notice.
- `runnerlinks/tests.rs`: 23, with the link-down tests at `:674` and `:947`. `runnerlog/tests.rs`: 9.
- `behaviour.rs`: 29 behaviours, each run on three targets. The macro at `behaviour.rs:4137` produces `::in_a_docker_pool` (`:421`), `::in_a_remote_docker_pool` (`:443`) and a local-link target.
- Others: `poolfollow`, `dockerpools`, `clusterpools`, `poolsetuptab`, `runnercode`, `runnerfiles`, `runnerwrites`, `hostfailing`, `hostlost`, `awayrunners`, `hostedsessions` (3400+ lines of tests).

### D.4 Web: vitest and stories (`app/src`)

About 60 `*.test.ts(x)` files mention pools or runners. The relevant ones:
- Pool setup: `PoolSetupTab.test.tsx`, `AddDockerPool.test.tsx`, `AddAnotherPool.test.tsx`, `poolsetup/{ConnectStep,TryStep,IdentityStep,SignInsStep,ClusterSteps,GetHelp}.test.tsx`.
- Notices: `RunnerLinkDownNotice.test.tsx`, `RunnerGoneNotice.test.tsx`, `RunnerKeyChangedNotice.test.tsx`, `PoolPinChangedNotice.test.tsx`, `PoolMintingNotice.test.tsx`, `HostLostNotice.test.tsx`, `MoveNotice.test.tsx`.
- Remote runner chats: `awayRunners.test.ts`, `chatMoves.test.ts`, `RunnerDispatchTab.test.tsx`, `RunnerWriteTab.test.tsx`.

About 30 stories mention pools, including `PoolSetupTab`, `poolsetup/*`, `RunnerLinkDownNotice`, `PoolPinChangedNotice`, `settings/Pools`, `HeldRunnerNotice` and `KeptRunnerNotice`. These run in Chromium and WebKit with keyboard and axe checks. Screenshots are drawn in CI only (CLAUDE.md).

### D.5 e2e / scenario specs (`app/e2e/specs`)

There are five pool specs, one `it` each, run by `wdio.pool.conf.ts` in the Linux `scenario` job's pool shard (`ci.yml:1881`, `matrix.pool` at `:2032`):
- `pool-setup-docker.pool.e2e.ts` (local Docker).
- `pool-setup-remote-docker.pool.e2e.ts` (dind behind sshd; reads fixture from `up.sh`).
- `pool-setup-remote-docker-tls.pool.e2e.ts`.
- `pool-setup-remote-podman.pool.e2e.ts`.
- `pool-setup-cluster.pool.e2e.ts` (a bare kind cluster from `tests/kind-cluster/up.sh`, `ci.yml:2164-2177`).

Each is a happy-path walk from Add a pool through Try it to a real chat. None injects a fault after the pool is up. Other specs (`first-task`, `stress`, `saving`, `projects`, ...) match "pool|remote" only incidentally.

### D.6 Real infrastructure in CI: what is real and what is faked

Real:
- **Docker daemon.** The `parity` job (`ci.yml:582-710`, ubuntu-24.04) builds a runner image of this build and pushes it to a local registry. It runs the `behaviour::*::in_a_docker_pool` suite plus the four real-docker `purlis-core` tests. It also runs the Linux sandbox's listen test inside a container under the runner's seccomp profile (`ci.yml:~648-690`). `nextest -E 'not test(/::in_a_docker_pool$/)'` keeps those out of the main `rust` job (`ci.yml:383`).
- **sshd + dind.** `remote-parity` (`ci.yml:719`, 60 min, ubuntu-24.04) runs `tests/remote-docker/up.sh`. It starts `docker:28-dind` with TLS on 127.0.0.1:2376 and a dockerised sshd on 127.0.0.1:2222 sharing dind's socket volume. Latency and loss are real kernel netem: `tc qdisc add dev eth0 root netem delay 40ms 10ms loss 1%` (`tests/remote-docker/up.sh:56`). It uses an agent key, a registry inside dind, and doctor-fault images (riscv64, fake-full disk).
- **Rootless Podman behind sshd** (`tests/remote-podman/up.sh`, run via `as-another-machine.sh`).
- **sshd with no Docker** (`tests/ssh-runner/up.sh`) and **a systemd machine behind sshd with logind sessions** (`tests/ssh-runner/up-systemd.sh`). The latter allows a real `docker restart` of a "machine".
- **kind.** `.github/actions/kind` is used by `operator.yml:166-217` (`operator/e2e/kind.sh`) and by the `scenario` pool shard (`ci.yml:2164-2177`) for a bare cluster of the oldest supported Kubernetes. Cluster tests are `#[ignore]` elsewhere.
- **Docker/Podman host matrix.** `docker-pool-hosts.yml` runs `docker on apparmor`, `podman rootful` and `podman rootless` on ubuntu-24.04, nightly and on PRs touching `dockerpool*`.
- **Linux 6.12.** `ci.yml:218-226` says the sandbox needs Landlock ABI 6 (kernel 6.12) and the tests never skip. The job prints the runner's `uname -r` and Landlock ABI as a notice. `ci.yml` also has an `ubuntu-26.04` entry (the matrix at `os [xcode-27, ubuntu-26.04]`), but I did not confirm its kernel. I found no pinned or self-provisioned 6.12 host; the job relies on the hosted runner image.

Faked or never run for real:
- `crates/stand-in` is NOT a fake runner. It is a test helper whose only job is to write an executable script without hitting `ETXTBSY` (`crates/stand-in/src/lib.rs:1-30`; `docs/development.md` layout table).
- `crates/fake-harness` fakes Claude Code or Codex output (a recorded corpus or synthetic output, hooks, an `--interactive` loop; `src/main.rs:1-20`). The runner image of every parity/scenario job is the real runner (`purlis serve`) plus the fake harness (`tests/parity/runner.Dockerfile`). So the harness, the model and the vendor are faked, while the runner host, link, sandbox and container are real.
- The pool-hosts job uses a `busybox` stand-in image whose `purlis` script only `sleep`s on `serve` (`docker-pool-hosts.yml` step "A stand-in image, by digest").
- `crates/host`, `session-protocol` and `routedsessions` unit tests use fake hosts and fake connectors. The link over real ssh to a real `purlis serve` is covered only by the single-test files in D.1.
- There is no SELinux host (`docker-pool-hosts.yml` header), no Windows or macOS remote path, and no sshd with a real jump host, `ProxyCommand` or `AllowTcpForwarding no` aside from what `a_remote_docker_pool_is_reached_over_ssh` configures.
- `remote-parity` is `continue-on-error` on push (`ci.yml:719-725`), so it is "evidence" there. On a PR it is gated through the `parity` scope step.

Gaps in the existing real-infra tests: the one-`#[test]` files (`ssh_runner_*`) hold sequential multi-step scenarios, so a fault in step N hides N+1. There is no measured-time assertion anywhere in the remote tests apart from the timeouts. `--test-threads 1` is forced because the tests mutate the shared sshd.

### D.7 Proposed smallest harness

**Reuse, do not build.** The fixture already exists: the `tests/ssh-runner/` systemd image (`up-systemd.sh`, sshd with real `purlis serve --install`) plus `up.sh`'s netem. The smallest addition is a **fault-injection helper plus one new test file on the existing `remote-parity` job**:

1. `tests/remote-faults/faults.sh` (about 40 lines of shell, run via `docker exec` on the existing containers `sshd`, `dind`, `ssh-runner-systemd`). Verbs:
   - `drop-link`: `iptables -I INPUT -p tcp --dport 22 -j DROP`, or `tc qdisc change ... loss 100%`.
   - `restore`.
   - `restart-sshd`: `docker exec ssh-runner-systemd systemctl restart ssh`.
   - `rotate-hostkey`: the existing `sshd-entrypoint.sh` already regenerates the key, as the host-key test shows.
   - `kill-runner`: `docker kill` / `pkill -9 purlis`.
   - `slow`: `tc qdisc change ... delay 15s`.
   - `break-registry`: `docker stop registry` inside dind.
2. One new test, `crates/purlis-core/tests/it/a_chat_on_a_remote_pool_survives_what_the_link_does.rs`. It uses the `an_ssh_runner_host_outlives_*` helpers (`linked`, `chat`, `running`, `until_within`; lines 196-277) and takes its fixture from env like the other tests. It runs inside the existing `remote-parity` job after "A runner's host outlives the sign-out", using `--test-threads 1`. It adds a CI step and an env flag `REMOTE_FAULTS_FIXTURE` that must be set, so a missing fixture FAILS (fix the silent-skip pattern for the new file).
3. No new service containers or GitHub Actions `services:`. The existing `up*.sh` scripts need `--privileged`/`NET_ADMIN`, which `services:` cannot grant. Marginal cost: about 3-6 minutes inside a 60-minute job.

### D.8 Regression cases, each mapped to the code path it exercises

Already covered (keep):
- host key change: `a_host_key_other_than_the_pinned_one_is_refused...` and `a_pool_whose_host_key_changed_is_trusted_again...` (`sshroute/connect.rs` `Finding::KeyChanged`; `sshroute/pool.rs` pin check).
- image unreachable / unsupported arch / full disk: `doctor_names_each_fault::{image_unreachable,unsupported_architecture,full_disk}` (`poolsetup/doctoring.rs` `DOWNLOADS`, `PLATFORM`, `ROOM`).
- whole-machine restart and sign-out: `an_ssh_runner_host_outlives...` (`runnerhost.rs`, `purlis serve --install`).

New cases for the new file:

| # | Case | Injection | Path exercised | Assertion |
|---|---|---|---|---|
| 1 | Link drop mid-session | `drop-link` for 5 s, then 90 s, with a chat typing | `session-protocol/src/hostlink/keep.rs:164-178` (`keep_linked`, pause 250 ms to 30 s), liveness ping (30 s, ADR 0078 2026-10-14), `sshroute.rs:614-616` ServerAlive 15 s x 3, `sshroute/forward.rs` watch, `sshroute/pool.rs:524-531` retry 5 s to 120 s | The chat's pty keeps its scrollback and accepts input within X s of restore. `link_down` carries a reason. `link_back` arrives. After 30 s `runner_links_down` is non-empty (the `RunnerLinkDownNotice` source). |
| 2 | sshd restart | `restart-sshd` while a chat is live | `sshroute/forward.rs` re-open ("A forward that dropped is opened again by the next hold", `:11`), `sshroute/pool.rs` `Failing` backoff | Forward reopens; no new host key prompt; chat survives (runner host is systemd-kept). |
| 3 | Host key change under a live link | `rotate-hostkey`, with the chat running | `connect.rs` `Finding::KeyChanged`, then `keep.rs:149-152` `key_changed`, then the key-changed Notice (`RunnerKeyChangedNotice.tsx`) | Link stays down. Notice names the key. Re-trust restores. |
| 4 | Runner kill | `kill-runner` (SIGKILL of `purlis serve`) | `runnerhost.rs` settle-from-record, ADR 0078 2026-11-17 "runner stopped by restart is started by the next chat", `routedsessions` | Chat is reported ended or suspended, not "Reconnecting…" forever. The next chat starts a new host. |
| 5 | App restart restore | kill the window process while chats run remotely | `routedsessions/chatskept.rs` (`ChatsKept` in the machine store, #4590), `a_window_that_comes_back_takes_back_its_runner_chats` | Each chat is re-attached to its original pool, one pool per chat. |
| 6 | Pool forgotten | `purlis pool remove` while a chat runs, then start | ADR 0078 2026-11-03 "dispatch naming a pool not kept is refused at once", `routedsessions` `RunsOn` | Refused at once with the pool named; the old chat is still found on its pool (`chatskept`). |
| 7 | Slow start | `slow` (15 s delay) on `ssh-runner-systemd` | `runnerlinks.rs:1377-1390` `ANSWER_WITHIN` 20 s, `CHECK_WITHIN` 60 s, `FIRST_LINK_WITHIN` (60+2xHANDSHAKE+10 s), `sshroute.rs:45-51` CONNECT 10 s / RUN 30 s / FORWARD 30 s | Link comes up inside the budgets. With the delay above a budget, it fails with a named cause, not a hang. |
| 8 | Image pull failure | `break-registry`, then start a chat that needs the pull | `poolsetup/doctoring.rs` `image-unreachable`, `dockerpool` pull, `runnerlinks.rs` `CHECK_WITHIN` | The start fails within `CHECK_WITHIN` with the "image unreachable" finding, and leaves no runner container behind (`image-leftovers.yml`, `leftovers.rs`). |
| 9 | Half-open link (packet blackhole, no RST) | `tc loss 100%` without closing | Liveness (30 s) and ServerAlive (~45 s) | Detected within the stated bound; this is the one the 2026-11-17 incident (5 min "Reconnecting…") looked like. |

---

## E. Observability today

### E.1 Logs and where they are

- **Diagnostic log:** `crates/purlis-core/src/applog.rs`. `tracing` events go to `charter.<YYYY-MM-DD>.log` in the app log dir and to stderr (`applog.rs:1-30`). It rotates daily, keeps 7 files (`DAYS_KEPT`, `:101`), and every event is scanned for secrets before writing. The dir is macOS `~/Library/Logs/dev.purlis.app` or Linux `~/.local/share/dev.purlis.app/logs`, and `PURLIS_LOG_DIR` moves it (`applog.rs:53-57`, `75-83`). The log's file name still reads `charter.` (legacy name) and `PURLIS_LOG_DIR` is the only knob. I found no `RUST_LOG`-style level switch for the user and no verbose or debug mode for the remote path. The filter is fixed (`what_a_log_keeps`, `applog.rs:237`: charter's info and up, libraries' warnings).
- **The CLI has no log file** (`applog.rs:13-16`). Its terminal shows warnings only.
- **`purlis serve` on a runner:** its stderr is its log, which is the service journal or the container log (`applog.rs:18-22`). In the CI the `ssh-runner` fixture reads `/home/runner/serve.log` (`ci.yml` failure step). A person with an SSH-installed runner has `journalctl --user` (systemd unit) or none.
- **`session-host.log`** (`crates/purlis-core/src/hostlog.rs`): launchd appends the host's stderr, size-capped at 1 MB with 3-ish copies; the doctor row `session host log` fires only if it is 2x over cap (`doctor/host_log.rs:1-14`).
- **`slow.jsonl`** (`docs/plane-format.md:9118`): window commands answering over 1 s and frames stalled over 200 ms. It never includes commands that "wait on a person or the network by design" (`answerWatch.ts` `WAITS_BY_DESIGN`), so a hung remote start does not appear there if it is listed as waits-by-design. I did not check whether the remote start is on that list.
- **A runner's own event log** is the device's audit chain (ADR 0078 section 7; `app/src-tauri/src/runnerlog.rs:1-30`). The window holds only run ends (max 1024 lines / 128 KiB) in memory, never on disk, and never writes them to its own log.

### E.2 Tracing and metrics coverage on the remote path

Counting `tracing::{warn,info,error,debug,trace}!` in non-test code:

| Area | Count |
|---|---|
| `crates/purlis-core/src/sshroute*` (`connect.rs`, `forward.rs`, `pool.rs`) | **0** |
| `tlsroute` | 0 |
| `runnerlink.rs`, `runnerinstall.rs`, `runnerhost.rs` | 0 |
| `poolsetup/*`, `doctor/*` | 0 |
| `crates/host/src`, `crates/session-protocol/src` (links, relay, `hostlink/keep.rs`) | 0 |
| `dockerpool.rs` | 2 (`:3379`, `:4164`) |
| `clusterpool.rs` | 1 (`:2378`) |
| `pools.rs` | about 30 (mostly warn about stores not kept) |
| `app/src-tauri/src/runnerlinks.rs` | 4 (`:140`, `451`, `483`, `910`) |
| `app/src-tauri/src/routedsessions.rs` | 5 (`:1098-1708`) |
| `app/src-tauri/src/sessionhost.rs` | about 25 (the best covered, for the local host's lifecycle: `:341-1050`) |

Spans: none. `#[instrument]`, `info_span!` and `tracing::span` show up only in tests (a counting subscriber in `hostedsessions/tests.rs:3394`, browser tests). Metrics: no metrics crate, and none intended (`applog.rs:34-35` "Not the audit and not telemetry (O1, ADR 0075)"). Telemetry-free is a stated rule, so any timing must be local (log or jsonl).

Consequence: the entire path from ssh spawn, host-key probe, forward, `docker exec`, handshake and hello to session proof runs with no logged event at its start, end, duration or retry. The ADR 0078 amendment of 2026-11-18 says the core "logged, again and again, why the link would not come up". That is the only logging of link tries I could find, and it is the `link_down` reason delivered to the window, not a trace line. I found the reason string built at `session-protocol/src/hostlink/keep.rs:152-154` (`"the connector did not connect: {why}"`) and sent on a channel. The relay does not log it, so only the window Notice (or the event stream) carries it. I did not find the "logged" claim in `host` or `session-protocol` source; it may be in `app/src-tauri/src/hostedsessions.rs` or the relay (not traced).

### E.3 Status surfaces and diagnostic commands

For a person:
- **`RunnerLinkDownNotice`** (`app/src/RunnerLinkDownNotice.tsx`): after `QUIET_FOR_MS = 30_000` (`:14`) it shows "Pool X's runner is not linked", the host's last reason, how long the chats waited, **Try now** (`link_now`, protocol 1.92), **Check pool** (opens pool setup) and **Dismiss**. It is read again every 15 s (`SAID_AGAIN_MS`, `:19`). Before 30 s each chat's pane says only "Reconnecting…" (`app/src/reconnecting.ts`). ADR: `docs/adr/0078/2026-11-18-a-runner-link-down-past-a-blip-says-why-and-offers-try-now.md`.
- **Other Notices:** `RunnerGoneNotice`, `RunnerKeyChangedNotice`, `PoolPinChangedNotice`, `HostLostNotice`, the session-host "failing" Notice (3 exits within a window, `sessionhost/breaker.rs`, `sessionhost.rs:770-790`, Try again button), `PoolMintingNotice`, `RestoringPane` ("purlis is starting this chat again", `RestoringPane.tsx:17`).
- **Pool setup tab:** `Connect` step (`poolsetup/connecting.rs`) shows each doctor Finding with a fix and "See what this does" commands. `Try it` shows the measured link round trip: "A docker command on {host} answers in {ms} ms" (`doctoring.rs:376-378`; median of 3, `:356-364`). `PoolStep.tsx:556` says "Making the pool and its runner. The first time, this downloads the runner." with no progress. `AddDockerPool.tsx:317` shows "Making its runner…". No step timing and no per-step progress other than the Pool-setup steps themselves.

For an agent or a terminal (CLI, `crates/purlis-cli`):
- `purlis pool doctor <name>` (`pool.rs:129`; `poolsetup::doctor`): checks the machine and engine, exits 1 with the finding. Refused inside a chat ("which a chat doesn't reach", `pool.rs:189-192`), so an agent chat cannot diagnose a hung remote pool, only the person can.
- `purlis pool setup [name] [--json]` (`pool.rs:117-124`): each step's state and what to do next, read-only (also refused inside a chat, `:186-188`).
- `purlis pool left-behind [--remove]`, `purlis pool limits/limit`.
- `purlis doctor [--json] [--preflight]` (`main.rs:397`): the plane's checks, plus runner-host rows only when run on a runner: `runner host service`, `runner host after sign-out`, `runner host` (`doctor/runner_host.rs:1-12`) and `session host log`. A desktop's doctor has no row about its pools or links.
- `purlis status` (`main.rs:312`): workspaces and clones only, nothing about pools or links.
- `purlis report bug --panic`: drafts a bug report from the newest panic (`report.rs`); crash minidumps via `crates/crash-handler` (#5691) go to the Report intake. Neither collects the diagnostic log, the link state or pool findings. I did not find a "diagnostic bundle" command. `docs/spec.md` has no such term.
- `crates/cluster-doctor` (lib only, 520 lines, no bin target found) is the cluster's check; its findings reach the user through Pool setup's cluster steps, not a `purlis` subcommand I could find.

When a remote session hangs, then:
1. The pane shows "Reconnecting…" over a dimmed last screen for 30 s, with no cause.
2. After 30 s a window-level Notice with the last reason (a free-text English string), Try now, Check pool.
3. For a hang that is not a link down (slow start, a runner that answers but does not progress), there is no Notice. The window's remote waits have named deadlines (`runnerlinks.rs:1377-1390`: 20 s answer, 60 s check, about 100 s first link; `sshroute.rs:45-51`: 10/30/30 s) that end in a refusal message, but no running indicator or elapsed time is shown while waiting.
4. The person's next step is Pool setup's Check again, `purlis pool doctor`, then the log file: which for the ssh/link path says nothing (E.2).

### E.4 Error shape

- Findings are a typed enum, `sshroute/connect.rs:23-` (`NoSuchName`, `NoAnswer`, `NotSsh`, `NotMet`, `NotMetThrough`, `KeyChanged`, `SignInRefused`, `NoEngine`, `NotAllowed`, `EngineSilent`, forwarding refused, ...) with doctor causes as string keys (`CONFINES="unconfined"`, `PLATFORM`, `DOWNLOADS`, ...; `doctoring.rs:36-41`). This is good and structured on the pool-check side, and it is rendered to English in `poolsetup/connecting.rs::said`.
- The link-layer error is not structured: `Down { key_changed: bool, gone: bool, why: String }` (`hostlink/keep.rs:146-155`). Only two booleans are typed. Everything else is a prose string, which the window forwards verbatim after `shown_safely`. ADR rule: the window "decides nothing from it" (2026-11-18 item 2). This follows the repo rule that nothing parses output, but it means no machine-readable cause, no attempt count and no timestamp per attempt: the ADR says "no time of each try is carried" (item 2).
- Several errors cross as `String` / `Result<_, String>` through `runnerlinks.rs`, `runnerlink.rs` (e.g. `runnerlink.rs:51`, `67`, `107`, `138`) and `pools.rs`.

### E.5 Gaps (observability)

1. **No log events at all on the ssh/forward/link/install path** (0 `tracing` calls in `sshroute*`, `tlsroute`, `runnerlink*`, `runnerinstall`, `host`, `session-protocol`). A "Reconnecting…" outage leaves nothing in `charter.<date>.log` to read afterwards. The ADR for 2026-11-18 says the core logged it; I could not find where.
2. **No spans, no durations, no step timing.** `Instant::now()` is used for deadlines and for the doctor's round trip only. No per-step time for: ssh meet, forward open, engine check, image pull, `docker exec` start, handshake, session proof. Pool setup `Making the pool and its runner…` has no sub-steps or byte progress, only static text (`PoolStep.tsx:556`).
3. **No correlation id.** A chat has a ULID and a number, a pool has a name, and a link has a device id, but no attempt or link id ties a UI Notice, a log line and a runner-side line together. grep for correlation/trace id in `docs/spec.md`, ADR 0075 and `CONTEXT.md` found nothing.
4. **Unstructured link-down reason** (E.4). No attempt counter on the link (`ups` is internal, `keep.rs:~160`), so the Notice's "trying at most 30 seconds apart" is stated, not measured. The forward's `Failing { failures, found_at }` (`sshroute/pool.rs:~535`) does carry a count but is internal and is not shown.
5. **No jitter in the reconnect backoff** (`keep.rs:178`: `pause * 2`, capped at 30 s, no randomisation). Many windows linking to one runner would retry in step. Two backoffs coexist: the link's 250 ms to 30 s and the forward's 5 s to 120 s with a 300 s finding cache (`pool.rs:524-531`); neither is visible in the UI.
6. **No user-facing verbosity switch.** There is no `RUST_LOG` or `--verbose` that raises the filter. `ssh` runs with `LogLevel=ERROR` and `BatchMode=yes` (`sshroute.rs:603-617`), so ssh's own diagnostics (auth methods tried, jump host failure) are discarded unless `connect.rs` captures them (it keeps `said` for a few Findings only).
7. **A chat cannot diagnose its own pool** (`pool doctor` and `pool setup` are refused in a chat). No agent-readable `purlis pool status` or `purlis runner links` showing link up/down, last reason, last try, retry in.
8. **Desktop `purlis doctor` and `purlis status` are silent about pools and links**; the runner-host rows exist only on the runner.
9. **No diagnostic bundle.** `purlis report` carries a panic; nothing collects the app log, the pool findings, the link state or the runner's `serve.log`. Session-host log capped 1 MB.
10. **Window keeps the runner's log only in memory** (by design, `runnerlog.rs`); after a restart nothing remains of why the last link went down. `slow.jsonl` keeps no link events.
11. **Silent pass on a missing fixture** in the real-infra tests (D.1). The pool specs fail without the fixture (`pool-setup-remote-docker.pool.e2e.ts` header), but the Rust ones return.

## Top problems found

1. The whole remote link path (ssh, forward, link keep-alive, runner install, relay) emits no log events and no spans (0 `tracing` calls in `sshroute*`, `tlsroute`, `runnerlink*`, `runnerinstall`, `crates/host`, `crates/session-protocol`). A hung or flapping remote session leaves nothing in the diagnostic log.
2. No timing per step, no progress for slow steps (image pull, runner make, handshake), no elapsed-time indicator. "Reconnecting…" has a 30 s quiet period and then one free-text reason.
3. Link-down reason is a plain `String` (`hostlink/keep.rs` `Down`) with no cause code, attempt number, timestamp or correlation id. Backoff has no jitter, and two independent backoffs (link, forward) are invisible to the person.
4. No fault injection on a live remote chat in CI. Real sshd/dind/systemd/netem fixtures exist (`tests/remote-docker`, `tests/ssh-runner`, `remote-parity`), but only a latency profile of 40 ms/1 % loss is applied. Link blackhole, sshd restart under a live chat, runner SIGKILL, app restart restore, forgotten pool, slow start and image pull failure under a chat are not exercised end to end.
5. The real-infra Rust tests silently pass when their `REMOTE_*` fixture env is absent. `remote-parity` is `continue-on-error` on push (`ci.yml:719-725`).
6. Agents and chats cannot diagnose a pool (`purlis pool doctor` / `setup` refuse in a chat), and desktop `purlis doctor` / `purlis status` say nothing of pools or links. There is no diagnostic bundle that gathers the log, pool findings and link state.
7. `stand-in` and `fake-harness` are not fake runners: the runner host and sandbox in parity are real, while the harness and model are faked. No Linux 6.12 host is provisioned by the repo; it relies on GitHub's hosted image, with the kernel only echoed as a notice.

---


---

# F. Tracker hygiene for M44, M44.1 and M18.1

**Not done: the tracker could not be read from the audit sandbox** (see the top of this file). No issue was closed or edited. What exists instead:

- Open counts: M44 has 125 open of 449, M44.1 has 12 open of 35, M18.1 has 29 open of 41.
- **Done-in-code, so probably done-but-open if an issue exists** (a person should check each against the tracker):
  - #7116 remote pool returns after its machine restarts, #7122 forget a pool whose machine is silent, #7130 forward lock released within 200 ms of ssh ending, #7135 runner link down past 30 s says why with Try now. All merged; look for older issues describing "stuck on Reconnecting", "cannot remove a pool on a dead host", "stale forward lock" and "no reason shown when the link is down".
  - #6955 (Connect separates "not SSH" from "key not saved"), #6463 (forwarding refused is named), #6205 (full disk is plain) are merged.
- **Likely open and still real**, so a tracker search should find or file them (these are the audit's findings A1 to A9, B.4, C.4, E.5): the SSH runner add path, the cluster vouch-wait dead end, kubeconfig and `PATH` stripping, unbounded kubectl calls, Remove on a remote pool, container-name collisions, the Try now backoff, the instant refusal while Down, the re-send after reconnect, and the missing logging.
- To finish F, run this outside the sandbox and give the output to a chat:

```
for m in "M44 · Runner pools" "M44.1 · SSH & provider runners" "M18.1 · Session host follow-ups"; do
  gh issue list --repo purlis/purlis --milestone "$m" --state all --limit 500 \
    --json number,title,state,updatedAt,labels,closedAt > "$m.json"; done
```

  Then match each open issue's title against the lists above and against `git log --grep '#<n>'`. The backlog-sync skill (`skills:backlog-sync`) does the evidence ledger.

---

# Top 15 problems, ranked

Ranked by how much each costs a person who wants to trust remote for real coding: stuck first, then slow, then silent, then untested.

1. **The SSH runner has no add path.** `runnerinstall::{probe,install}` are called only from tests; `Kind` has no SSH-host variant (`crates/purlis-core/src/poolsetup.rs:57-64`); there is no `runner add` command. ADR 0078 and the module docs read as if it shipped (A1).
2. **A chat start makes two full links.** The profile-readiness link (`app/src-tauri/src/chats.rs:5301` -> `poolprofiles.rs:867` -> `runnerlinks.rs:926`) is thrown away, then the relay makes its own (B3).
3. **Every link runs up to seven serial `docker` commands first, and the container check is not cached.** `docker info` three times, `container inspect` twice, a volume inspect and an exec, each over the SSH forward (`runnerlinks.rs:205-208`, `dockerpool.rs:3214,3880,4056,4140`). The engine check has a 30 s cache; this one has none (B6).
4. **The ssh, forward and link path logs nothing.** Zero `tracing` calls in `sshroute*`, `tlsroute`, `runnerlink*`, `runnerinstall`, `crates/host` and `crates/session-protocol`; no spans, no durations, no step timing. A hang leaves nothing to read (E.2).
5. **Try now cannot beat the failing-forward backoff.** After a forward fails, `docker_through` answers from memory for 5, 10, 20, 40, 80, then 120 s (`crates/purlis-core/src/sshroute/pool.rs:524-531`); only Pool setup's Check again clears it (`poolsetuptab.rs:397`). The Notice promises "at most 30 seconds apart". No test covers it (B.4, C.4 #1).
6. **A start during "link down" on a Docker pool is refused at once** instead of waiting a few seconds, because the wait logic only knows cluster pools (`crates/host/src/routing.rs:208-248`). After a blip the person sees "Runner not linked… send this again" (B.4, C.4 #3).
7. **No fault injection on a live remote chat in CI.** Real sshd, dind, systemd and netem fixtures exist (`remote-parity`, `ci.yml:719`), but nothing drops a link, restarts sshd, kills a runner or slows a start under a running chat (D.6, D.8).
8. **Real-infra Rust tests pass silently when their fixture is absent**, and `remote-parity` is `continue-on-error` on push (`ci.yml:719-725`). Green does not prove they ran (D.1, D.6).
9. **In the Pool setup tab a slow cluster cannot be added.** The vouch wait can only be raised in the old no-project dialog; the tab sends `vouch_wait: null` (`ClusterSteps.tsx:501-504`) and the failed add forgets the pool (`pools.rs:2537`), so the "raise it to 10 minutes" advice has nowhere to go (A3).
10. **Cluster setup cannot name a kubeconfig, and `KUBECONFIG` and most of `PATH` are stripped** (`clusterpool.rs:872,838-842`; `ClusterSteps.tsx:497`). Many real setups see an empty context list (A2).
11. **kubectl calls in add and install have no timeout and no Cancel** (`clusterpool.rs:928-935`; `install.rs:323-343`). A credential plugin waiting on a browser leaves the spinner forever (A4).
12. **Removing a remote pool is a dead end for most failures.** "Forget it here" is offered for three findings only (`connecting.rs:640-650`); sign-in refused, Docker gone, forwarding refused and no engine leave a bare error (`dockerpools.rs:606`, `runnerlinks.rs:100`) (A6).
13. **Reconnecting re-sends whole repositories.** `forget(sent)` (`hostedsessions.rs:3966`) discards what the runner holds, and clones go one at a time (`runnercode.rs:537`), so the first start after an outage is the slowest (B10).
14. **A cold start pays a full SSH handshake and holds a global lock while it does.** `ControlMaster=no` (`sshroute.rs:600`); the forward opens lazily inside the first start while holding the global `HELD_HERE` mutex for up to 30 s (`sshroute/forward.rs:363`), blocking other pools. Nothing pre-warms on opening a workspace (B4).
15. **A person and an agent cannot see why a pool is down.** The link-down reason is a prose string with no cause code, attempt count or time (`hostlink/keep.rs:146-155`); `purlis pool doctor` and `pool setup` are refused inside a chat; desktop `purlis doctor` and `status` say nothing of pools; there is no diagnostic bundle (E.3 to E.5).

Also real but ranked lower: container names carry no desktop id so two desktops on one engine collide (A7); the old dialog skips the image-contract check (A5); swallowed errors in the first-start path and the Connect lists (A8); `~/.ssh/config` `Include` is ignored by the host picker (A9); reboot recovery has no end-to-end test (C.4 #2); no jitter in the reconnect backoff (E.5 #5).

---

# Open decisions (questions only the person can answer)

Each is phrased to become a wayfinder ticket.

1. **Is the SSH runner a product or a library?** Do you want a person to add a bare SSH host as a pool kind in the window (install over SSH, keep it running, pair), or is a remote Docker engine over SSH the only remote you ship? If yes, which comes first: window path or `purlis runner add`?
2. **How fast must a remote chat start, warm and cold?** `docs/spec.md` has no budget for a pool start. Give a number at a stated round trip (for example: warm under 2 s at 40 ms, cold under 6 s) so the work has a target, and say whether it is a CI-gated budget or a goal.
3. **May a start reuse one link, even if the profile keep then happens on the relay's link?** That changes ADR 0078's separation of the profile path from the relay. Is that separation a requirement or an accident?
4. **May the container check be cached for a short time (as the engine check is, 30 s), and for how long?** The risk is a runner removed in the last seconds; the cost today is up to seven serial commands per link.
5. **What should "Try now" mean?** Does it clear the forward's failing memory and force a fresh try, always? Or is the 5 to 120 s backoff a deliberate protection of the remote machine that Try now must respect?
6. **Should a start during a short "link down" on a Docker pool wait, and for how long, before refusing?** Cluster pools already wait. Is the instant refusal for Docker pools a decision or an omission?
7. **Pre-warming: may purlis open a pool's SSH forward and link when a workspace that uses it is opened, before any chat starts?** It spends a connection and a few `docker` runs on a machine the person has not asked for yet.
8. **Should the forward use SSH multiplexing (ControlMaster)?** ADR 0123 (2026-11-17) rejected it for lifecycle reasons. Does that rejection still hold given a cold start costs a full handshake every launch?
9. **After a reconnect, may the first start ask the runner what code it already holds instead of re-sending every repository?** The protocol can already report it; the current code assumes nothing.
10. **What is the supported way to point a cluster pool at a kubeconfig?** Setup deliberately drops `KUBECONFIG` and limits `PATH`. Is that a security rule to keep (then say what to do for people with several files or credential plugins), or a default to relax with a field?
11. **A pool whose machine answers but refuses (sign-in refused, Docker gone): may the person forget it here, as with a silent machine (#7122)?** Today Remove is a dead end for those findings.
12. **What is the logging contract for the remote path?** Which events must always be logged (forward open, each check step with its duration, each dial attempt with cause and attempt number), at what level, and for how long are they kept, given the no-telemetry rule (ADR 0075)? Is a per-person verbose switch allowed?
13. **May an agent chat read pool status and run the pool doctor?** Both are refused inside a chat today. Is that a security boundary or only a default? If it stays, what read-only view may an agent have?
14. **What does "trust remote" mean as a test you can run?** Which faults must be survivable in CI before you would use it for real work: link drop, sshd restart, runner kill, host reboot, app restart mid-outage, forgotten pool? Should `remote-parity` and the new fault suite block merges (today it is `continue-on-error` on push), and may real-infra tests fail instead of skipping when their fixture is missing?
15. **Which terminal states get their own words?** A runner gone, a pool forgotten, a key changed and a machine unreachable are today partly merged for the chats on them. Do chats on a forgotten pool say "ended with that runner" or something distinct, and may they be moved to another pool?
16. **Who answers for two desktops on one remote engine?** Container names have no desktop id (`dockerpool.rs:695-702`). Do you want one pool per desktop per engine, or a shared runner with several desktops? (This decides whether to rename or to warn.)
17. **Where does the SSH-runner path stand against the cluster path for effort?** If effort is limited, which of "SSH runner add path", "cluster fixes (kubeconfig, vouch wait, timeouts)" and "remote Docker speed and reliability" goes first?
