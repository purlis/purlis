# One SSH connection per pool: OpenSSH multiplexing or `russh`

Research note, 2026-11-18, for the wayfinder map "Remote you can trust". Read-only: no product code
changed. Paths are relative to the repo root.

How to read the tags: **[code]** read in this checkout, with `file:line`. **[man]** read in OpenSSH
10.0p2's own pages on this machine (`man ssh`, `man ssh_config`; the OpenBSD web copy fetched too).
**[src]** read in OpenSSH's `openssh-portable` source on GitHub (`mux.c`, `clientloop.c`, `ssh.c`,
master branch, fetched today). **[docs.rs]** / **[crates.io]** / **[rustsec]** read today at the URL in
Sources. **[audit]** a figure from `docs/research/2026-11-remote-lifecycle-audit.md` on branch
`origin/remote-audit-pool-setup-chat-start-sessi-6z2w4c94`, which is itself an estimate. **[est.]**
my own estimate. **[unverified]** could not be checked.

## Question

Can purlis keep one SSH connection per pool alive and reuse it, either with OpenSSH's
`ControlMaster` / `ControlPersist` or with SSH inside purlis through the `russh` crate? Do ADR 0123's
lifecycle reasons for rejecting `ControlMaster` still hold?

## Short answer / recommendation

**Neither is worth doing for the chat-start latency; one small step with OpenSSH is worth doing for
the other ssh calls, and `russh` should wait.** purlis already holds one SSH connection per pool in
use: the `ssh -T -L` forward is the connection, held in a process-wide table and reused by every
`docker` run as a new channel (`sshroute/forward.rs:354-382`, `sshroute/pool.rs:148`). So a warm chat
start pays **zero** SSH handshakes and a cold one pays **one** (plus a local `ssh -G` spawn). The
audit's 7 serial `docker` spawns and its doubled link (B3, B6) are what make a start slow, and
neither option touches them. What reuse can still save is (a) the one cold handshake per app launch,
and (b) the one-shot `ssh` runs that each pay a full handshake because `ControlPath=none`: a Podman
pool's file reads at every container check, and the probes on the failure path. For (b), make the
forward's own `ssh` the master (`-M -S <path>`, no `ControlPersist`, ended by the same `HELD_BY`
pipe) and let the one-shot runs ride it with `ControlMaster=no -S <path>`. That keeps every property
ADR 0123 asked for. For (a), only a persistent master (`ControlPersist`) helps, and that is the one
place the ADR's rejection still holds in full. `russh` would end the child-process half of the
lifecycle (nothing can orphan) but not the socket half (`docker` still needs a unix socket), costs a
second RC-pinned crypto stack, and means writing what `ssh_config` gives for free.

ADR 0123 (amendment `docs/adr/0123/2026-11-17-a-forwards-lock-never-outlives-its-ssh.md:36-48`),
item by item:

| ADR reason | Still holds? |
| --- | --- |
| `docker` needs a local socket, `-W` reaches only `host:port` | **Half stale.** True of `-W` ([man] ssh `-W`). But a master takes forwards at run time with `ssh -O forward` ([man] ssh `-O`), and `mux.c` carries unix-socket paths in them (`mux.c:583-710`, `listen_path` / `connect_path`) **[src]**. So a master could own the `-L`. Whether it should is a different question: the lock stays either way. |
| `ControlPersist` leaves a master apart from its opener, an orphan by design, and the parity job's orphan check fails on it | **Holds**, for `ControlPersist`. A killed or crashed holder leaves the master up until its idle timer ends it or someone sends `-O exit` ([man] ssh_config `ControlPersist`). **Does not apply** to a master without `ControlPersist`, which closes with its first client ([man] ssh_config `ControlPersist`: "will close as soon as the initial client connection is closed"), and which the existing `HELD_BY` pipe already ends. |
| Two processes racing on `ControlMaster=auto` can each end up with their own connection | **Holds for `auto`**, and is avoided by taking the master only under `held.lock`. The failure is silent: a second master finds the path taken and says "ControlSocket %s already exists, disabling multiplexing", then carries on as a plain connection (`mux.c:1378-1408`) **[src]**. |
| `ControlMaster=no`, `ControlPath=none` are already passed | **Not a reason.** It is a policy statement (`sshroute.rs:600-601`, pinned by the test at `sshroute/tests.rs:132-148`), and ADR 0067's chat-sandbox copy of it (`docs/adr/0067/2026-10-10-04-tunnels-for-clients-that-skip-the-proxy.md:42`, `crates/purlis-core/src/sandbox/tunnel.rs:836`) is about a *chat's* ssh and stays. |

The audit's one-liner "rejected for lifecycle reasons ... not a quick win" (audit B.3, item 14) is
right for `ControlPersist` and for the cold start, and too broad for a master that lives and dies
with the forward.

## What the code does today

**One long-lived connection per pool, per holding process.** `hold_noting` (`sshroute/forward.rs:363-382`)
returns the kept forward while it is alive (`Forward::alive`, `:145-148`, or, for a forward another
process holds, `shared_alive`, `:349-351`: the lock is held and the socket answers a connect). It
opens one only when there is none (`open`, `:172-265`). Every `docker` run for the pool is pointed at
that socket (`pool.rs:148-193`, `pointed` `:294-315`), and each run is a new channel on the same
connection (ADR 0123 amendment of 2026-11-12, "Each run is pointed at the forward").

**Spawns and handshakes per chat start**, from the code and the audit's call chain:

| Step | Process spawns | SSH handshakes | Source |
| --- | --- | --- | --- |
| Forward already held and checked within 30 s | 0 `ssh` | 0 | `pool.rs:170` (`checked_lately`, `ENGINE_CHECKED_FOR` = 30 s) |
| Forward already held, engine not checked lately | 0 `ssh`, 1 `docker info` | 0 | `pool.rs:171-194`, `connect.rs:597-625` |
| Cold: no forward | 1 `ssh -G` (`connect::resolve`, local, no network), then 1 `ssh -T -L` | **1** (TCP, key exchange, auth, one `exec sh -c` through the login shell) | `forward.rs:191-192`, `:201-209`, `connect.rs:236-237` |
| Cold start of a second process (`purlis pool …` beside the window) | 0 `ssh` | 0: uses the holder's socket (`Held::Shared`) | `forward.rs:187-189`, `:308-339` |
| Forward silent after a remote reboot | 1 more `ssh -T -L` | 1 | `pool.rs:256-292`, ADR amendment of 2026-11-17 (forward gone silent) |
| Forward will not open, or engine silent on a fresh forward: diagnosis | `ssh -G`, `-W` check, probe (1 to 3 `ssh`) | 1 to 3 | `connect.rs:535-595`, `pool.rs:493` |
| Podman pool, every container check: `is_file`, `read`, `keep`, `let_go` | 1 `ssh` each | **1 each** | `dockerpool.rs:3897-3905`, `sshroute.rs:1066-1155` |
| Docker pool container check, the 6 to 7 serial `docker` runs, the exec, the link | 0 `ssh` | 0 | `dockerpool.rs:3880+`, [audit] B6 |

So the answer to "how many handshakes per chat start" is 0 warm, 1 cold, and N for a Podman pool
(N = reads and probes in its check, every link, because every one is its own `ssh` with
`ControlPath=none`). I did not count the Podman N exactly: it depends on which files `check_oci`
asks `is_file` about **[unverified]**.

**Latency.** No timer covers any step ([audit] header, B.0). The audit's estimates: a forward open
adds +0.5 to 1 s at 40 ms RTT and +1.5 to 3 s at 150 ms (with the `docker info`); a warm start is
about 3 to 4 s at 40 ms and 8 to 12 s at 150 ms, dominated by the doubled link and the 7 serial
`docker` runs ([audit] B.2). **[est.]** (the audit's, not mine). **I could not measure `ssh` here.**
A local sshd would have been needed, starting one was refused by the session's permission classifier,
and I did not retry or work around it. Nothing in this note is a measured latency.

**Lifecycle machinery that exists because `ControlMaster=no`** (all stays or changes under each
option below):

- `ServerAliveInterval=15`, `ServerAliveCountMax=3`: a silently dead network is noticed in about 45 s
  (`sshroute.rs:55-56`, `:615-616`; [man] ssh_config `ServerAliveCountMax`: "approximately 45 seconds").
- `HELD_BY` (`sshroute.rs:632`): the forward's remote command waits on a pipe from purlis, so a
  holder that dies, however it dies, ends the connection.
- `held.lock` plus a 200 ms watch (`forward.rs:109-134`) and a connect-to-answer test (`:344-351`)
  from #7130.
- `socket_of` refuses a path longer than 103 bytes (`sshroute.rs:42`, `:903-917`).

## Comparison

| | OpenSSH multiplexing | `russh` in the app |
| --- | --- | --- |
| **Cold-start cost** | Master with no `ControlPersist`: unchanged, one handshake per app launch (the forward is the master). Master with `ControlPersist`: the launch after the first, inside the idle timeout, pays none; the process spawn and `ssh -G` remain. Saves each one-shot `ssh` of the Podman and failure paths (1 handshake each) | One handshake per process per pool, no `ssh` spawn and no login shell; one-shot runs become channels. No saving across an app restart (the connection dies with the process) unless a broker process lives on |
| **Failure** | Dead master: socket file stays; a `ControlMaster=auto` client that gets `ECONNREFUSED` unlinks it (`mux.c:2434-2437` **[src]**); a plain client (`ControlMaster=no`) does not, and only reports it. Network drop: `ServerAlive*` ends the master after about 45 s, as now. Master gone with clients riding it: their sessions end | No ssh to die, no stale-pid reuse; the `Handle` reports `is_closed` ([docs.rs] `Handle`) and `Handler::disconnected` fires ([docs.rs] `Handler`). Keepalive is `Config.keepalive_interval` and `keepalive_max`, defaults to be set by purlis ([docs.rs] `Config`). A local listener for `docker` is still a file a crash leaves behind |
| **Cleanup** | No `ControlPersist`: ends with the holder via the existing pipe. With `ControlPersist`: nothing ends it but its idle timer or `-O exit`; a killed or crashed app leaves it. The control socket is a second file to remove (temporary `<path>.<16 chars>` first, then `link()` and `unlink`: `mux.c:1360-1408` **[src]**) | Ends with the process: the TCP connection closes, the server ends the session. No `HELD_BY`. The `docker` socket file remains (see forward lock) |
| **macOS / Linux** | `ssh` on both. `sun_path` is 104 on macOS and 108 on Linux (the repo says 104, `sshroute.rs:40-42`; **the 108 is [unverified]**). `ControlPath too long` is a `fatal` (`mux.c:2417-2420` **[src]**), and ssh adds 17 bytes for its temporary name. With the data dir `…/Library/Application Support/dev.purlis.app` a `…/engine-links/<pool>/mux` path is 97 bytes for pool `docker` and 103 for an 11-letter pool, so it fits only just **[est.]** (computed from the path, not measured). `%C` is a 40-hex hash ([man] ssh_config TOKENS: "Hash of %l%h%p%r%j"; the width is **[unverified]** here) and would not fit in that directory, so a hashed path needs a short directory | Pure Rust over tokio; `ring` or `aws-lc-rs` backend. macOS and Linux agents by `AgentClient::connect_env` (`SSH_AUTH_SOCK`) or `connect_uds` ([docs.rs] `AgentClient`); Windows by Pageant ([crates.io] Cargo.toml). `aws-lc-rs` is the default feature and brings `aws-lc-sys` (a C build); `ring` is already in `Cargo.lock` (0.17.14) |
| **Forward lock** | No master: unchanged. Master = the forward's ssh: unchanged, plus the control socket under the same lock. `ControlPersist` master: the child is detached, so rule 1 of #7130 (the holder watches its own child) has nothing to watch; see below | The lock's job (one holder per socket) shrinks to an in-process mutex only if the processes stop sharing; the window, the session host and `purlis pool` do share today. A local `UnixListener` for `docker` keeps the dead-socket problem |
| **Keys, agents, `known_hosts`** | All of the person's `ssh_config`, `ssh-agent`, `IdentityAgent` (1Password), `ProxyJump`, via `ssh` as today; purlis's pinned `known_hosts` alias scheme (`sshroute.rs:538-540`, `:546-560`) unchanged. A client riding the master uses the master's login | None read from `ssh_config`; `ssh -G` (already run, `connect.rs:236`) could still resolve hostname, user, port, identity files, jump hosts. Host key: `Handler::check_server_key` (default rejects all) with the pin compared in code. Agent: `AgentClient`. Passphrases and `ProxyJump` (a channel to the jump host used as the stream) are purlis's to write. `IdentityAgent` is not read by `russh` **[unverified, from the docs.rs listing]** |
| **Security** | Socket mode 0600 (umask 0177, `mux.c:1368`), in a 0700 dir; any process of the same user can ride a master and run commands as the person on the remote. The existing `-L` socket (`StreamLocalBindMask=0177`, `sshroute.rs:149`) already gives the same user a root-equivalent Docker API. `purlis`'s chats must not reach it (engine-links is held from chat sandboxes, ADR 0123 D-6257-1) | No ride-along socket besides the `docker` one. The attack surface is `russh` itself: seven RustSec advisories, six issued 2026-11-12 (see below) |
| **Dependencies / unsafe policy** | None new. No purlis `unsafe` | New: `russh` 0.65.2 with `ssh-key =0.7.0-rc.12`, `ed25519-dalek 3`, `rsa 0.10` and others, beside the lock's `ssh-key 0.6.7` and `ed25519-dalek 2.2.0` (`Cargo.lock:1946,8216`). `russh-cryptovec` uses `unsafe` ([README]). The rule "no unsafe beyond five audited blocks" is enforced on purlis's own sources (`crates/purlis-core/tests/it/five_unsafe_blocks.rs:111-154`), which a dependency does not break; whether the operator reads the rule to cover dependencies is a question for them **[unverified]** |
| **Effort [est.]** | Master without persist: small (a few arguments in `forward_args`, a control path, and one-shot runs given `-S`), about 1 to 2 days with tests. `ControlPersist`: a few days and an ADR amendment | Weeks: connection, pinned key check, agent, keys with passphrases, `ProxyJump`, keepalive, reconnect, a unix listener proxy, a broker or per-process connections, tests with a real sshd, the audit surface |

## Option 1: OpenSSH multiplexing

**What the manual says [man].** `ControlMaster yes` makes ssh listen on `ControlPath`; further
sessions with `ControlMaster no` reuse it "but will fall back to connecting normally if the control
socket does not exist, or is not listening". `ControlPersist` backgrounds the master after its first
client closes; with a time it ends after that long idle; with `yes` or `0` it stays until killed or
`-O exit`. Control commands: `check`, `forward`, `cancel`, `proxy`, `exit`, `stop` (`ssh -O`). Agent
and X11 forwarding over a mux use the master's. `ServerAliveInterval` / `ServerAliveCountMax` apply to
the master's connection as to any ([man] ssh_config; the master is an ordinary connection).

**What the source adds [src].**
- A master listens on `<ControlPath>.<16 random chars>`, then `link()`s to the final name and unlinks
  the temporary one, so no one meets a half-ready socket (`mux.c:1352-1408`). If the final name exists
  it logs "ControlSocket … already exists, disabling multiplexing" and continues as a plain
  connection. That is the silent-fallback hazard for two racing masters.
- A client that finds the socket and gets `ECONNREFUSED` unlinks it only when its own
  `ControlMaster` is not `no` (`mux.c:2434-2437`). A stale file left by a killed master therefore
  stays until a `yes`/`auto` ssh runs on that path; purlis would need to remove it itself, under its
  lock.
- The `ControlPersist` idle timer is cancelled while `channel_still_open` and set when a client
  connection closes (`clientloop.c:246-262`, `:1661-1664`). Whether an idle `-L` listener counts as an
  open channel is **[unverified]**.
- `ControlPersist` is incompatible with `UpdateHostKeys=ask` (`ssh.c:1359-1362`); purlis passes `UpdateHostKeys=no`
  (`sshroute.rs:593`), so this does not bite.

**Failure modes.**
- *Host reboot or network drop:* the master's own `ServerAlive*` ends it after about 45 s, the same
  bound the forward has today. Until then the control socket still answers `-O check`, so a "master
  answers" test is no better than the existing `answers()` connect test (`forward.rs:344-346`) at
  finding a master that has lost its far end. That is the problem ADR 0123's third amendment
  (`2026-11-17-a-forward-that-went-silent-is-opened-once-again-before-a-start-refuses.md`) solved by
  asking the engine.
- *App killed or crashed:* without `ControlPersist`, the `HELD_BY` pipe closes and the remote `sh`
  kills its sshd, so the connection ends with every channel on it (`sshroute.rs:621-632`, ADR 0123
  amendment of 2026-11-12). Unchanged by making that ssh a master. With `ControlPersist`, the master
  has no pipe from purlis (it is backgrounded after auth, `ssh.c:2893`), so a crash leaves it, its
  control socket and its `-L` socket until the timer or a later purlis calls `-O exit`. Who cleans:
  only the next purlis start, which would have to find it by path under the lock.
- *Two processes (window, `purlis pool`):* today the second uses the holder's `-L` socket. A master
  adds nothing here; only the holder runs `-M`.

**Smallest change that keeps ADR 0123's properties.** In `forward_args` add `-M -S <dir>/mux` and
drop `ControlMaster=no` / `ControlPath=none` for that one ssh, keep the `HELD_BY` pipe, and give the
one-shot runs (`forward_check_args`, `probe_args`, `read_args`, `is_file_args`, `keep_args`,
`let_go_args`, `sign_in_args`) `-S <path>` with `ControlMaster=no`, falling back to today's fresh
connection when the master is not there (the manual's own fallback). The pin still holds because
riders use the master's already-checked key; the riders' own `StrictHostKeyChecking` options are
unused. A rider with the wrong path silently opens a plain connection (still pinned, as today).
This saves a handshake per one-shot run. It does not save the first one, nor one `docker` spawn.

**With `ControlPersist`** (needed to save the cold handshake across app restarts): the forward would
be started with `-M -S <path> -o ControlPersist=<n>s` and then `-O forward -L …` through the control
socket. The ADR's orphan reason applies: the parity job's orphan check would have to allow a master
that lasts up to *n* seconds after the holder, and the window must call `-O exit` on a clean quit.
It saves, per app launch within *n*, one handshake (estimated 4 to 6 RTT plus auth, [audit] B4, i.e.
0.5 to 1 s at 40 ms RTT). It is the choice only if the person values that and accepts a bounded orphan.

## Option 2: `russh`

**State [crates.io] / [rustsec].** `russh` 0.65.2, published 2026-11-16, Apache-2.0, MSRV 1.89 (purlis
pins 1.99.1, `docs/research/build-time-and-memory.md`). Ten million downloads. Twelve releases since
2026-08-11, including breaking 0.x bumps. Repository `Eugeny/russh` (also shown as `warp-tech/russh`):
last push 2026-11-16, 82 open issues, not archived (GitHub API). Pre-1.0.

**Supports [README, docs.rs].** Client and server; `publickey`, `password`, `keyboard-interactive`,
OpenSSH certificates; `direct-streamlocal` (client), so `channel_open_direct_streamlocal` reaches the
engine's socket; OpenSSH keepalive and strict key exchange; `mlkem768x25519-sha256`. Handshake and
lifetime: `client::Config` has `keepalive_interval`, `keepalive_max` and `inactivity_timeout`;
`Handle::is_closed`, `send_ping`, `disconnect`; `Handler::check_server_key` (required, default rejects
all) and `Handler::disconnected`. Agent: `keys::agent::client::AgentClient` (`connect`,
`connect_env`, `connect_uds`). Known hosts: `keys::known_hosts::{check_known_hosts, learn_known_hosts}`
and `_path` forms; purlis would not use them, since the pin is compared in `check_server_key`.

**What it does not do.** It reads no `ssh_config` (nothing in its modules does, **[unverified]** beyond
the listing). `ProxyJump` and `ProxyCommand` are the caller's: open the jump connection, take a
channel, hand it to `connect_stream`; `ProxyCommand` has no equivalent. `IdentityAgent`, `Include`,
`Match`, 1Password's socket path and `AddKeysToAgent` are `ssh`'s. `ssh -G` can keep resolving
`hostname`, `user`, `port`, `identityfile`, `proxyjump` and `identityagent` for it (`connect.rs:236`),
but purlis would then implement what it prints. I could not read the issue tracker (GitHub's search
API rate-limited the sandbox), so issues about agent, `known_hosts`, keepalive and `ProxyJump` are
**[unverified]**.

**Lifecycle.** A `Handle` lives with its tokio task. When the process dies the socket closes. No
`HELD_BY`, no orphan, no `held.lock` for the connection itself, no `ssh` pid to reuse. What remains:
the unix socket purlis must serve for the `docker` CLI (a `UnixListener` proxying each accept to a
`channel_open_direct_streamlocal`). A crash leaves that file, so the "does something answer" rule
(#7130 rule 2) and `StreamLocalBindUnlink`-like unlinking stay. And sharing across the window, the
session host and `purlis pool` needs either one process serving the socket (a new broker) or one
connection per process (a handshake per process, as today's second process avoids).

**Dependencies [crates.io, docs.rs, `Cargo.lock`].** `russh` needs `ssh-key =0.7.0-rc.12` (an exact
release candidate), `ed25519-dalek ^3`, `rsa ^0.10`, `p256 ^0.14`, `digest ^0.11`, `signature ^3`.
`Cargo.lock` has `ssh-key 0.6.7`, `ed25519-dalek 2.2.0`, `rsa 0.9.10`; so the tree would hold two of
each, with a pre-release ssh-key in the signing-adjacent path of a security product. Some of the
rest is already there at the new versions (`curve25519-dalek 5.0.0`, `aes 0.9.3`, `p256 0.14.0`,
`sha2 0.11.1`, `digest 0.11.3`, `pkcs8 0.11.0`, `signature 3.0.0`), so the increment is smaller than
the list. Crypto backend: `aws-lc-rs` is the default and adds `aws-lc-sys` (a C library built by the
crate, not in `Cargo.lock` today); `ring` is the alternative and is in `Cargo.lock` (`ring 0.17.14`),
and the README says to keep `flate2` and `rsa` when choosing it. `deny.toml` bans multiple versions of
a few crates only (`minidump-*`); `multiple-versions = "allow"`, so duplicates are permitted.

**Advisories [rustsec].** Seven for `russh` since 2024 (one reported in August 2024), six of them
issued 2026-11-12 (six days ago):

| Id | What | Patched in | Side |
| --- | --- | --- | --- |
| RUSTSEC-2024-0450 (CVE-2024-43410) | packet length not bounded before allocation, pre-auth | 0.44.1 | both |
| RUSTSEC-2026-0154 (CVE-2026-46673) | agent frames unbounded, server and client | 0.60.3 | agent client too |
| RUSTSEC-2026-0496 (CVE-2026-42189) | keyboard-interactive response count allocation, pre-auth | 0.60.1 | server |
| RUSTSEC-2026-0494 (CVE-2026-48110) | message fields allocated before size check, pre-auth | 0.61.0 | both |
| RUSTSEC-2026-0493 (CVE-2026-46702) | no limit on a decompressed packet | 0.61.1 | both, with compression |
| RUSTSEC-2026-0492 (CVE-2026-102823) | client `Handler` callbacks run for channels never opened | 0.63.1 | client, integrity |
| RUSTSEC-2026-0495 (CVE-2026-116794) | agent **server** signs confirm-constrained keys without asking | 0.65.0 | server only |

Five are denial of service from a hostile peer, which for purlis means a malicious or compromised
remote machine making the desktop allocate without bound. One (0492) lets a server drive client
callbacks. All are fixed in 0.65.x, so 0.65.2 is clean as of today. It means a steady flow of
security releases from a pre-1.0 crate that purlis would own the upgrade cadence for. OpenSSH has its
own advisories, but a person already has `ssh` patched by their OS.

## Forward lock under each option

The lock today (`forward.rs`): one holder per pool holds `held.lock`; it lets go of the socket then
the lock the moment its ssh ends (200 ms watch, `:109-134`, `:95-105`); another process uses a held
forward only while its socket answers (`:344-351`); a lock over a dead socket is waited out for 2 s
(`:302`, `:308-339`).

- **Master without `ControlPersist`.** The forward's ssh is the master. Lock, watch, `answers()` and
  `let_go_of_all` are untouched. The control socket joins `socket` in `Lease::let_go` (remove it
  before releasing the lock, as the `-L` socket is, `forward.rs:97-105`). A killed holder leaves two
  files, not one; the next holder removes both after taking the lock. Another process (`Held::Shared`)
  does not ride the master (it would need the path and the same pin; it uses the `-L` socket as now).
  Unchanged: the 103-byte limit, which now applies to the control path plus 17 bytes
  (`mux.c:1360`).
- **Master with `ControlPersist`.** The ssh child is detached, so `Lease::ended` / `try_wait` (`:91-93`)
  watches a process that exits right after authentication. The holder's identity becomes "the master
  `-O check` names" (it prints the pid, `mux.c` `Master running (pid=…)`). Rule 1 of #7130 (the
  holder lets go within 200 ms of its ssh ending) would have to be redone around `-O check` polling.
  Rule 2 (use a shared forward only while it answers) stays. A new rule is needed for the holder that
  is gone but whose master persists: take over (`-O forward` again, or `-O exit` then a fresh `-M`)
  under the lock. ADR 0123's amendment of 2026-11-12 ("every forward ends with the process holding
  it") would need its own amendment.
- **`russh`.** The ssh child and `HELD_BY` go. `Lease` holds a task handle and a `UnixListener`.
  `watch` becomes `Handle::is_closed` or `Handler::disconnected`, an event instead of a 200 ms poll.
  `held.lock` stays if more than one process serves a socket for the pool; a crash still leaves the
  socket file and rule 2 still applies. The wait for a dead holder (`LET_GO_WITHIN`) shortens only
  if the OS closes the listener, which it does for a process that dies, but not the file.

## Open questions and what would settle them

1. **Is the cold handshake worth a bounded orphan?** Measure a start with the forward cold and warm
   (no timer exists, [audit] header). Add a `tracing` span around `hold_noting` and the `docker info`
   check (the audit's gap E.2) and read a day of real starts, then compare the cold-only cost with the
   7-spawn chain. If cold is a small share, `ControlPersist` is not worth the exception.
2. **How many one-shot `ssh` runs does a Podman link make?** Count `is_file`, `read`, `keep`,
   `let_go` calls in one `check` on a rootless Podman pool (`dockerpool.rs:3897-3905`). This decides
   whether the master-without-persist step pays.
3. **Does an idle `-L` listener hold `ControlPersist`'s timer open?** (`clientloop.c:246-262`, whether
   `channel_still_open` counts listeners.) A five-minute test with `ssh -v` against a local sshd.
   That is the measurement this note could not make; it needs `sshd` on localhost.
4. **Does a long data-dir path fit?** The `Application Support/dev.purlis.app` base leaves 103 - 97 = 6
   bytes of pool name beyond `docker`, and the existing `docker.sock` already has room for 9. A user
   with a longer home folder fails `socket_of` today; a control path adds 17 bytes. Decide a short
   directory (such as `$XDG_RUNTIME_DIR` or a hashed name under a short root) before any multiplexing.
5. **`sun_path` on Linux is 108?** Read `man 7 unix` on a Linux host; unverified here (macOS host).
6. **Does the operator read "no unsafe" to cover a dependency?** `russh-cryptovec` has `unsafe`, as do
   `ring` and `aws-lc-sys`. A ruling is needed before any `russh` work.
7. **Which `russh` issues bear on purlis?** Read the tracker for agent, `known_hosts`, keepalive and
   `ProxyJump` (the sandbox could not).
8. **Is the audit's B3 (the doubled link) and B6 (7 serial `docker` runs) fixed first?** They are the
   cost the person feels, and fixing them makes the SSH handshake's share visible.

## Sources

Repo:
- `docs/adr/0123-a-docker-pool-can-run-on-another-machine-reached-over-ssh-or-dockers-tls-api.md` (§2, D-6257-1 reasons 1 to 4, §8)
- `docs/adr/0123/2026-11-12-the-forward-is-held-by-a-pipe-and-docker-is-pointed-at-it-by-docker-host.md`
- `docs/adr/0123/2026-11-17-a-forwards-lock-never-outlives-its-ssh.md` (the rejection, lines 36-48)
- `docs/adr/0123/2026-11-17-a-forward-that-went-silent-is-opened-once-again-before-a-start-refuses.md`
- `docs/adr/0067/2026-10-10-04-tunnels-for-clients-that-skip-the-proxy.md:42`, `docs/policy-inventory-families.md:223` (N43), `docs/runner-image.md:195`: the only other hits for ControlMaster, all about a chat's own ssh or a runner's sockets. No hit anywhere in `docs/` or `crates/` for `russh`, `ssh2` or `libssh` (`ssh2` appears once, as a test string, `sandbox/planted.rs:4720`). `docs/spec.md` has no hit for any of them.
- `crates/purlis-core/src/sshroute.rs`, `sshroute/forward.rs`, `sshroute/pool.rs`, `sshroute/connect.rs`, `sshroute/tests.rs`, `sshroute/forward/tests.rs`; `crates/purlis-core/src/dockerpool.rs:3880-3925`; `crates/purlis-core/src/sandbox/tunnel.rs:836`
- `Cargo.lock`, `Cargo.toml` (lints, `unsafe_code = "deny"`), `deny.toml`, `crates/purlis-core/tests/it/five_unsafe_blocks.rs`
- Lifecycle audit: `git show origin/remote-audit-pool-setup-chat-start-sessi-6z2w4c94:docs/research/2026-11-remote-lifecycle-audit.md` (Part B, B.2, B.3 last row, B.4; E.2; Top problem 14; open decision 8)

External, read today:
- OpenSSH 10.0p2 `ssh(1)` (`-M`, `-S`, `-O`, `-W`) and `ssh_config(5)` (`ControlMaster`, `ControlPath`, `ControlPersist`, `ServerAliveInterval`, `ServerAliveCountMax`, TOKENS): local `man`; web copy https://man.openbsd.org/ssh_config (fetched, 200)
- OpenSSH source, https://raw.githubusercontent.com/openssh/openssh-portable/master/mux.c (v1.119, 2026/10/30), `clientloop.c`, `ssh.c`
- https://github.com/Eugeny/russh (README, `russh/Cargo.toml` at main); https://crates.io/api/v1/crates/russh
- https://docs.rs/russh/0.65.2/russh/client/struct.Config.html, `.../client/struct.Handle.html`, `.../client/trait.Handler.html`, `.../keys/index.html`, `.../keys/known_hosts/index.html`, `.../keys/agent/client/struct.AgentClient.html`
- https://rustsec.org/packages/russh.html and each advisory page named in the table
- https://api.github.com/repos/Eugeny/russh (repository facts); the search API was rate-limited, so no issue was read

Not reached or not done: a local sshd measurement (refused by the session's permission classifier,
not retried); `man 7 unix` on Linux; russh issues.
