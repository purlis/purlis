# Amended 2026-10-26: a chat may listen on a port

**Built by #2019** (spec #2008), on the ruling of 2026-10-14: listening on a port is a sandbox
grant a chat asks for on its first bind, and its port then becomes a named port of that chat's
browser. *Decided in implementation, to be confirmed.*

1. **A grant of its own, at two scopes.** `listen` (`grant::What::Listen`) is one port of this
   Mac's loopback, for *this chat* (in memory) or *this project on this machine*
   (`listens_mine` in the machine's sandbox-grants file, never committed). Never once, never
   every project, never a range, every port or one of purlis's own (`localport::port`). Held by
   the same policy lock as a port to connect to (`sandbox.socket-grants`), and audited as
   `what = "listen"`.
2. **The loopback alone.** Seatbelt is given `network-bind` and `network-inbound` on
   `localhost:<port>` and the connect rule for it, so the chat reaches the server it started. A
   server started on every interface (`0.0.0.0`, `::`) is still refused; the Notice and the line
   the chat is told say to start it on `127.0.0.1`.
3. **Its browser's named port.** The port is in the chat's reach as `127.0.0.1:<port>` and
   `[::1]:<port>`, at the layer that granted it, so the browser's lane carries it by its literal
   address with no second Ask (`Widened::listens` is a subset of `Widened::ports`). Removing it
   withdraws it from every open chat's proxy and browser, unless the chat or the project still
   holds that port to connect to or to listen on.
4. **Linux.** The wrap's supervisor takes `bind` as it takes `connect`: it reads the address
   once, judges it by the profile's `network-bind` rule, binds a new socket of its own to that
   address (with `SO_REUSEADDR` and the chat's non-blocking and close-on-start flags) and puts it
   in place of the chat's descriptor; the supervisor's Landlock layer binds only the profile's
   listening ports, and the chat's own layer still binds nothing. A refused bind is told as
   `network-bind <address>`. A port of zero and a unix socket are refused as before. Landlock
   holds a port but no address, which is why the supervisor makes the bind. The sandbox needs
   Landlock ABI 6 already, so there is no kernel where a listen grant applies without its rule;
   on an older kernel no wrapped chat starts. At the floor (`(allow default)`) a bind is now
   allowed on Linux as it is on macOS.
5. **Detection.** The sandbox's own line (`network-bind`, `network-inbound`), the Linux
   supervisor's refusal, and a failed command's own words (Node's `listen EPERM`, Go's
   `bind: operation not permitted`) are a Block of operation `listen` and kind `local-service`.
   In a Codex or opencode chat on macOS the port is printed words, so it pre-fills the Notice's
   field for the person to check.
6. **Claude Code.** Its sandbox's settings bind every loopback port (`allowLocalBinding`) or
   none, never one, so its Notice says so and offers Start without the sandbox.
