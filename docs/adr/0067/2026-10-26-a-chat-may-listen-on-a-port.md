# Amended 2026-10-26: a chat's server behind purlis's front door

**Built by #2019** (spec #2008), on the ruling of 2026-10-14: serving on a port is a sandbox
grant a chat asks for on its first bind, and its port then becomes a named port of that chat's
browser. The front door, its cookie jar and the Node shim were ruled by the person on 2026-10-26;
the rest was decided in implementation and approved in review.

1. **A grant of its own, at two scopes.** `listen` (`grant::What::Listen`) is one port, from
   1024, for *this chat* (in memory) or *this project on this machine* (`listens_mine` in the
   machine's sandbox-grants file, never committed). Never once, never every project, never a
   range or one of purlis's own; never a port something on this Mac listens on, which is asked
   about as a service to connect to instead, at the Ask, at the Allow and at each start. Held by
   the `sandbox.socket-grants` lock, and audited as `what = "listen"`.
2. **The chat's server never listens on a TCP port.** It listens on a socket of its own,
   `<port>.sock`, in a folder purlis keeps for the chat, `0700` and inside a `0700` folder of
   this user's (`purlis-serve/` in `XDG_RUNTIME_DIR`, or this process's temp folder;
   `sandbox::door::folder_for`). A browser cannot reach a unix socket, and no other user can
   enter the folder.
   - On macOS the chat's profile names that socket alone (`network-bind`, `network-inbound`,
     `local unix-socket`), and no TCP bind.
   - On Linux the supervisor binds it: a bind of that socket's path, and a bind of an internet
     address on the port, at any address, are each made on the socket instead, so a dev server
     that asks for its port is served through the door unchanged. Any other TCP bind is refused
     and told, raising the Ask.
3. **purlis's front door** (`sandbox::door`) listens on `127.0.0.1:N` and `[::1]:N` while the
   chat runs, in the window's process wherever the chat's proxy runs.
   - It refuses a request with `421` unless its `Host` is `localhost:N`, `127.0.0.1:N` or
     `[::1]:N`.
   - It refuses one with `403` when an `Origin` is sent that is not one of those three origins.
   - It strips the browser's cookies. The cookies the chat's server sets are kept in a jar of
     the door's own, one for each browser and port, told apart by a cookie of its own
     (`purlis-door-N`), and handed back to the server alone. A request the browser says another
     page of this machine sent (`Sec-Fetch-Site` of `same-site` or `cross-site`) uses no jar and
     is given none. A cookie longer than a browser keeps is not kept, and each jar is held to a
     size.
   - It carries a request only to the socket bound at the chat's own path, in the folder as it
     was when the door opened: a link or a second name put there, or another folder put in its
     place, is answered `502`.
   - On macOS it also holds the port on every interface, bound and never listened on, so a
     server started there later is refused its bind, as on Linux.
   - It carries an upgrade (hot reload) once checked.
   - It streams, and carries one request per connection, with a body by its `Content-Length`
     alone.
4. **Its browser's named port**: the port is in the chat's reach as `127.0.0.1:N` and
   `[::1]:N`, so the browser's lane carries it to the door with no second Ask; the chat's own
   commands connect to it there too, only once its door opened: a port that gets no door, for
   something listening there already or any other reason, is withdrawn from the chat's proxy and
   is in no rule of its profile. A Claude Code chat's door carries to connections its server
   dials in ([amended 2026-10-30](2026-10-30-a-claude-code-chat-s-server-dials-in.md)). No chat
   writes in the folder the sockets are in but its own socket. Removing the grant withdraws it
   from every open chat's proxy and browser and closes its door there at once (2026-10-30), and
   the chats serving there restart once their turn ends.
5. **`purlis listen N -- <command>`** runs a server there. It hands the command
   `PURLIS_SERVE_SOCKET`, `PURLIS_SERVE_PORT`, `PORT` and `HOST`, and for Node a shim on
   `NODE_OPTIONS`. The shim (`crates/purlis-cli/src/listen/shim.cjs`) moves a listen on the
   port onto the socket and has the server say it listens on the port. It is the one Node
   preload shim, by the person's ruling of 2026-10-26, as an exception to the rule against
   mixing languages in the shipped app.
6. **Claude Code**: its sandbox binds every port or none. On macOS its server dials in to its door
   instead ([amended 2026-10-30](2026-10-30-a-claude-code-chat-s-server-dials-in.md)), and its
   Notice offers Allow. Elsewhere its Notice says so and offers Start without the sandbox (#3316).
