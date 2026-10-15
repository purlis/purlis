# Amended (2026-10-10, #1667; amends section 3): tunnels for clients that skip the proxy

The operator's decisions of 2026-10-10 on spec #1661 (N-4 and N-9): a client that opens its own
TCP connection and never asks a proxy (psql, usql, mysql, redis-cli, ssh) reaches an allowed host
without the chat changing its command. It took over #1637's first line, which was escalated
there.

1. **A database host through a tunnel, pointed at by `secret exec`.** When `purlis secret exec`
   hands a command a value from a vault that names one place a client connects to (a database
   URL of a scheme whose client speaks plain TCP, a libpq `host=… port=…` string, or a bare
   `host:port`), and the chat may reach that exact host and port, purlis opens a port on the
   loopback interface that carries every connection to exactly that host and port, and hands
   the command the value pointed at it. The run's sandbox lets the command connect to that port
   besides its proxy's two. The vault is what ties the tunnel to the host, as `database:<vault>`
   in section 3 meant. Only the `--env` values are pointed; a `--file` or `--dotenv` file is
   handed as the vault holds it.
2. **Only a listing with that port opens one.** A host listed without a port (a preset's) is
   carried by the proxy on HTTPS's port, and never opens a raw tunnel on another. The decision
   is the core decision module's, with no default port, so the local-address check holds: this
   machine, a link-local address or a cloud metadata service is never a tunnel's target unless
   that exact address and port is listed.
3. **Never a proxy.** The target is fixed as the tunnel opens, from the vault's value, which no
   chat writes. Nothing a connection sends is read. Each connection resolves the name again and
   connects only where a chat may reach, as the proxy does.
4. **A value read two ways is left alone.** Several hosts, a host in a URL's query as well, an
   `@` after a URL's authority, a socket path, a scheme purlis does not know: the value is
   handed as it is, and the client fails as before.
5. **A host the chat may not reach is a Block**, told as the proxy tells one, so the chat's
   Notice offers Allow for that host and port. A host that carries one of the run's values is
   never named, as before. One on this machine is said in a note naming the variable alone.
6. **Every tunnelled connection is in the network record**, under the asking chat, as the
   proxy's connections are; so are the brokered run's proxy connections, which were not before.
   A host that carries one of the run's values is counted there and not named.
7. **A tunnel lives as long as the run** that opened it, which ends when the chat that asked
   goes. Ended, it stops listening and closes every connection it carries.
8. **ssh, and git over ssh, through the chat's SOCKS port.** Each chat purlis's sandbox wraps
   whole (Codex, opencode), and each brokered run, gets an ssh configuration in its own temp
   directory that sends every ssh connection through the chat's
   SOCKS port (with the person's own `~/.ssh/config` read after it, for names, users and keys),
   an `ssh` that reads it first on the chat's `PATH`, and `GIT_SSH_COMMAND` pointed at that
   `ssh`. No connection rides a master connection another ssh opened outside the sandbox
   (`ControlMaster no`, `ControlPath none`). A Claude Code chat's own sandbox sets
   `GIT_SSH_COMMAND` for each command through the same SOCKS port, which is purlis's since #1665.
   Its harness, its hooks and its servers run outside its sandbox and must never run a file from
   a folder a chat may write, so its route is written under the app's state, which no chat
   writes (`.purlis/app/ssh/`, one folder per chat, removed as it ends), and reaches only the
   commands its sandbox runs: the one file Claude Code reads before each command
   (`CLAUDE_ENV_FILE`) puts the route's `ssh` first on that command's `PATH`, after the
   person's shell start-up files, so a bare `ssh` takes it too. The harness's own `PATH` never
   names it, and it sets no `GIT_SSH_COMMAND`. A persona's tools (#1407) go before it in that
   same file. *(Amended 2026-10-12, #1667: before, a Claude Code chat was handed no route, and a
   bare `ssh` there did not take it, #1708.)*
   The SOCKS port decides by host and port, so a refused host is a Block. Signing is not
   the route's: it names no key and no agent, and the SSH agent purlis holds for a chat (#1350)
   comes through `SSH_AUTH_SOCK` unchanged. A chat that rewrites the file in its own temp
   directory reaches nothing more: its network is still its sandbox's.
