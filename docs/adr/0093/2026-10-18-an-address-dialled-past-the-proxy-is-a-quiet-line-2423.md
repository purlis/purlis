# Amended 2026-10-18: an address dialled past the proxy is a quiet line (#2423)

Follows [purlis's own wrap on Linux tells what it refused](2026-10-17-the-linux-wrap-tells-what-it-refused-2344.md).

On Linux, a program in a chat purlis wraps that connects straight to an internet address,
instead of going through purlis's proxy, is refused by the wrap's supervisor. That refusal is now
told, as a block of its own kind, `address`.

- **It names the address, and offers no Allow.** The proxy names the hosts it refuses, and a
  host is allowed only through the proxy. So an address a program dialled past it is no host
  anyone could allow, as a lookup a program makes past the proxy is not. The Notice says so and
  names the way round: a client that goes through the proxy.
- **A quiet line.** Nothing the person answers lets the connection through, so nothing waits on
  the answer, and the asks registry does not list it until the person takes it up.
- **Once a minute a chat.** The supervisor tells one such address a minute, whatever the
  address. It is counted apart from the ten other refusals a minute, so addresses never crowd
  out a file's or a socket's Notice.
