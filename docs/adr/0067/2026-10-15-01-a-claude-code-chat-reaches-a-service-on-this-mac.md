# Amended (2026-10-15, qw384b; extends the qw384 amendment): a Claude Code chat reaches a service on this Mac by name — ruled by the operator, 2026-10-15

Decided under delegation (option A of qw384's follow-up); ruled by the operator, 2026-10-15.

- **By name, through the chat's proxy.** Claude Code sends a command's connections to purlis's
  proxy, but its `NO_PROXY` keeps `localhost`, `127.0.0.1`, `::1` and the private ranges off it,
  and every Go client never proxies `localhost` or a loopback literal. A name it does send. So
  the chat's proxy carries the names of this Mac (a fixed list: `this-mac.internal`,
  `kubernetes.docker.internal` and `host.docker.internal`) on a port to `127.0.0.1:<port>`:
  - only while the chat holds that port (granted for it, or for every chat of the project on
    this machine), at the layer that granted it; on any other port it is this machine, refused
    as such and never asked about as a host;
  - never looked up: the connection is made to this machine's loopback on that port and
    nowhere else. No other name is looked up on this account, and any other name is decided as
    before;
  - each connection kept in the network record by the name it was reached by. The decision is
    the reach table's (`Reach::decide_by_name`); the proxy only asks it.
- **Removal.** Removing a port's grant stops every new connection to it from every open chat's
  proxy at once (but a chat that holds it for itself), by any name. A connection already open
  (a `kubectl get --watch`) runs until it ends or the chat restarts; the Seatbelt rule of a
  Codex or opencode chat goes at its next start, as before.
- **The Notice.** A Claude Code chat's port Block offers the same Allow as Codex and opencode,
  saying the name its commands reach it by. The chat restarts on its conversation to take it
  and is told the name.
- **kubectl: Point kubectl at it.** After the Allow, on the person's press, the Notice lists the
  contexts of their kubeconfig (as `kubectl config view` reads it) and copies the one they pick
  for that chat alone:
  - before a context is chosen, the Notice says the chat gets its own copy of it, its
    credentials included, which the chat can read;
  - only a context whose server is on this Mac (`127.0.0.1`, `localhost` or
    `kubernetes.docker.internal`, over HTTPS); any other is listed with why and cannot be pressed,
    and one that names a proxy of its own or skips the certificate check is refused;
  - its server becomes `https://this-mac.internal:<port>` and `tls-server-name` the name it had,
    so the certificate is still checked against the name it was made for;
  - the copy is written into the chat's own folder at each of its starts, made new each time and
    never through a link, readable by its owner only, named by `KUBECONFIG` for that chat alone, and removed when the chat ends; held in
    memory by the chat's id, as a chat's own grant is;
  - purlis never writes `~/.kube` or any kubeconfig of the person's. The chat restarts on its
    conversation to take it.
- **What it does not reach.** A program that uses no proxy (psql, redis-cli, mysql) cannot reach
  a service on this Mac from a Claude Code chat this way. A brokered run for those is a separate
  ticket.
