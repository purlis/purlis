# Amended (2026-10-14 and 2026-10-15, the operator's rulings on spec #2008 and #2131, #2011; extends section 3): a browser's reach is a lane of its chat's

Under the rulings of 2026-10-14 (one grant system, browser reach a lane of the sandbox's egress
grants, one Granted list), each browser has a **reach proxy** of its own
(`purlis_core::sandbox::browser_reach`), built from the chat's proxy (`sandbox::egress`): its
strict reading of a request's head, its tunnel and plain request, its limits.

1. **A lane, not a second list** (ruled 2026-10-14). The browser's lane is its chat's reach,
   read again by the browser's rules: a host the chat's reach lists is carried at the layer that
   lists it, and nothing is listed for the browser alone. A grant is made and removed as the
   chat's are, and shows in the one Granted list. *The lane never takes the floor's reach of a
   chat without the sandbox: a browser reaches the same whatever its chat's sandbox, so a public
   site nothing lists is a new site (item 6) and a private address is carried only where it is
   listed exactly (item 3). Decided in implementation by the dispatcher from the 2026-10-14
   ruling that a chat without the sandbox gets browsers like a sandboxed one, 2026-10-15, to be
   confirmed.*
2. **By host and port, without opening TLS** (ruled 2026-10-14), as the chat's proxy decides: a
   `CONNECT` by its target, a plain request by its absolute target and its one `Host`. *One HTTP
   port, no SOCKS: decided in implementation, 2026-10-15, to be confirmed.*
3. **Public addresses, and private ones by their literal address and port** (ruled 2026-10-14;
   the private rule ruled 2026-10-15, #2131). Never this machine (but for a named local port,
   item 4), a link-local or cloud metadata address, multicast, broadcast, or a reserved,
   documentation or benchmarking range. A private address (a private IPv4 range, the carriers'
   shared space, an IPv6 unique local address) is carried only as the literal address and port
   the person allowed for that chat or project, which the chat's reach lists exactly: never by a
   name, never by a range, never on another port, never in another spelling. An IPv6 form that
   carries an IPv4 address is judged as that address and is never a private address's
   spelling. This holds whatever the chat's reach lists.
4. **A named local port by its literal address alone** (ruled 2026-10-14): `127.0.0.1:N` or
   `[::1]:N`, where the chat's reach lists exactly that address and port. A name of this machine
   (`localhost`, a name under it, a name of this Mac) is refused. *A name that ends in a number,
   an address written another way, is refused outright: decided in implementation, 2026-10-15,
   to be confirmed.*
5. **The proxy resolves names itself, once**, and connects only at the public addresses that
   lookup answered. *A lookup is given up after 10 seconds, and where this machine's own
   addresses cannot be read the proxy carries nothing: decided in implementation, 2026-10-15, to
   be confirmed.*
6. **Public sites are readable; a site new to the project asks** (ruled 2026-10-14). As the
   glossary's **Reach** says, a site new to the project that the agent itself writes an address
   for asks once, and so does a form it posts there (#2018); a link the page offered does not.
   By host and port the proxy cannot tell these apart, so it tells every public site nothing
   lists to one seam, which the browser's broker answers. Until #2018 the seam refuses every
   new site, an interim default. *A site nothing lists is reached on HTTPS's port by a tunnel and
   HTTP's by a plain request, and refused on any other port: decided in implementation,
   2026-10-15, to be confirmed.*
7. **Fails closed** (ruled 2026-10-14). The engine resolves no name and sends every request to
   its reach proxy. When the proxy stops it ends every connection it carries, tunnels included,
   and lets its port go only once nothing can connect to it; every load then fails.
