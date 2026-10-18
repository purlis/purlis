# Amended 2026-10-18: a refusal at the system's name lookup socket is a lookup (#2428)

Follows [purlis's own wrap on Linux tells what it refused](2026-10-17-the-linux-wrap-tells-what-it-refused-2344.md).

On some Linux hosts a program's name lookup goes through a local socket: systemd-resolved's,
nscd's, sssd's or avahi's. No profile purlis compiles reaches one, at the floor or at the full
sandbox, because a chat's names are looked up by purlis's proxy. So such a lookup is refused at
that socket by the wrap's supervisor, and the program may also say it could not resolve its host,
which its hook tells.

- **It is a lookup past the proxy, not a socket to allow.** The supervisor's refusal at one of
  those sockets (`sandboxblock::RESOLVER_SOCKETS`) is the block a lookup past the proxy is,
  naming no host, since the supervisor never sees the name.
- **One failed lookup, one Notice.** A lookup that names no host is the same block as any lookup
  of that chat's within the throttle's minute, whichever was told first. Lookups of two named
  hosts stay two.
- **Its words say what reaching that lookup would mean.** The Notice says the program looks its
  host up itself, through the system's name lookup. Allowing the host would not let it through,
  and letting it reach the system's name lookup would let the chat look up any name past the
  proxy, so purlis offers neither. It names the way round: a client that uses the proxy.
