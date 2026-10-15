# Amended (2026-10-10, #1664; amends section 3): purlis runs the proxy, one pair of ports per chat

Delegated under the operator's decisions of 2026-10-10 on spec #1661 (N-7, N-8 and N-12):
purlis's own proxy carries the network of every chat it wraps, and decides by host and port.

1. **One proxy per chat, on a pair of loopback ports of its own**: an HTTP proxy (`CONNECT` and
   plain absolute-form requests) and a SOCKS5 proxy (`CONNECT`, no authentication). Both are
   made when the chat starts and close when it ends. A Codex or opencode chat's compiled
   sandbox lets it connect to those two ports and to no other address, so no chat reaches
   another chat's ports. Today the proxy runs in the process that starts the chat's terminal;
   it moves with chats into `purlisd` (ADR 0068).
2. **A connection's chat is the port it came in on.** Nothing a connection sends names a chat,
   and nothing it sends is read as one. This replaces the per-request token #1071 proposed,
   which needed every client to send it.
3. **One decision module** (`sandbox::reach`) answers every connection: open (a preset in force
   or the project's own hosts), persona, allowed (this project on this machine, or this chat),
   ask, or refused. **Ask is refused for now**, with a Block whose Notice offers Allow, until a
   connection is held while the person answers (#1666). Refused is what no person could allow:
   not a host, or a local address.
4. **The local-address check is the proxy's own.** A connection to this machine (loopback, the
   unspecified address, any of this machine's own addresses), a link-local address or a cloud
   metadata service is refused, by address and by every address a name resolves to, checked at
   each connect. The one exception is that exact address and port, listed. No list can name
   such an address today (a host is checked as it is added, #1341), so the exception is for the
   tunnels of #1667, which decide whether one may. Private ranges are hosts as before (#1341).
   Such a refusal is a Block that names no host, since no Allow would let it through.
5. **Host and port only.** No TLS interception, no certificate of purlis's own, no path rules.
   The bytes inside a tunnel are the client's.
6. **Every connection is in the network record** (#1662): each as a `connect` line with the
   host and port, the decision, and how many connections it stands for, coalesced to a line per
   host and port a minute so a chat cannot turn the record over; a refusal raises its Block
   besides.
7. **Bounded**: a connection cap per chat over both ports together, a deadline for a head or a
   SOCKS greeting, an idle timeout on every tunnel, and no buffer past a fixed size.
