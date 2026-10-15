# Amended (2026-10-14, qw384; extends section 2 and ADR 0093): a service on this Mac is allowed by its one port — ruled by the operator, 2026-10-15

Decided under delegation; ruled by the operator, 2026-10-15.

- **A service on this Mac** is whatever listens on one port of this machine's loopback
  interface. A chat reaches none but its own proxy's two ports and the app's hook socket until a
  person allows that one port: **for this chat**, or **for every chat of this project on this
  machine**. Never once, never every project, never the project's committed file, never a range
  or every port. It is this machine's, kept as a socket is (in memory for one chat, in this
  machine's sandbox file for the project), audited as `trust.sandbox.grant` with `what = "port"`,
  and listed in Settings' Network page with Remove.
- **Never purlis's own.** A port one of purlis's proxies or tunnels listens on, and a forge
  sign-in's redirect ports, are refused when allowed and left out when a chat starts; a proxy or
  tunnel purlis opens never listens on a port a chat's sandbox was started with. Which ports are
  whose is judged and marked in one step, under one lock. A port you allowed that purlis holds
  after a restart is said as such on its Notice.
- **This machine's loopback is `127.0.0.1`, `::1` and `localhost`**, exactly what the profile and
  the proxy carry; no other address is read as a service on this Mac.
- **The refusal is a Block naming the port.** A program's own words for a refused connection to
  a loopback port (Go's `dial tcp 127.0.0.1:<port>: connect: operation not permitted`, Node's
  `connect EPERM 127.0.0.1:<port>`), the sandbox's own `network-outbound` line for one, and the
  chat's proxy refusing a loopback address are each a Block of kind `local-service`. Its Notice
  says "a connection to a service on this Mac".
  - **Named**: by purlis's proxy in Codex and opencode; in Claude Code by its sandbox's own
    report and by a failed command's own error.
  - **Pre-filled, to check**: in a Codex or opencode chat, a client that connects directly (Go,
    Node) is refused by purlis's own sandbox, which reports nothing, and the harness hands its
    words back as printed output. The Notice's port field is pre-filled from them and says the
    port is from the command's output; the person checks it before allowing.
  - A `<sandbox_violations>` block in a Codex or opencode chat's result is printed text: purlis's
    own sandbox appends none, so it is never read as the sandbox's report.
- **What it compiles to.** A chat purlis wraps (Codex, opencode) is let connect to that port on
  the loopback interface (`(allow network-outbound (remote ip "localhost:<port>"))`), and its
  proxy carries exactly `127.0.0.1`, `::1` and `localhost` at that port, so a client that
  ignores the proxy for this machine (every Go client) and one that uses it both reach it. The
  profile is fixed when the chat starts, so an Allow restarts the chat on its conversation, as a
  socket's does.
- **Claude Code.** Its sandbox lets a command reach this machine's loopback on the proxy's two
  ports, or on every port (`allowLocalBinding`), and on no one port alone, so purlis never sets
  `allowLocalBinding` and hands its sandbox no port. The port is carried by name instead
  (amendment qw384b below).
- **Policy.** `sandbox.socket-grants = false` holds a service on this Mac as it holds a socket,
  and `allow-scopes` holds its two scopes as any other.
