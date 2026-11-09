# Amended 2026-11-09: a scoped link-local jump host, as ssh spells it

Decided under the operator's standing order to take the recommended path (D-5893-1, #5893, from
the ninth adversarial review of #5876), recorded here as a delegated decision. It follows *A jump
host's brackets hold an IPv6 address* (2026-11-09).

ssh expands its `%` tokens in a jump's whole command: `ssh_config(5)` gives ProxyJump `%%`, `%h`,
`%n`, `%p` and `%r`, and any other letter after a `%` stops ssh. `%r` is the user `-l` names,
which no rule here reads, so a `%` that forms a token could paste a value nobody judged into the
command a shell runs. Only `%%` stands for a `%` itself.

So a scoped IPv6 jump host is offered an Allow only when it is spelled as ssh reads a zone in a
jump, and only where no token can form:

- **inside the brackets, after a link-local address** (`fe80::/10`), as `%%` and a zone of
  letters, digits, `.`, `-` and `_`: `-J u@[fe80::1%%en0]`, with a port after it or not;
- **as the last hop of the jump list**, in `-J` or `-o ProxyJump`: the hops before it are pasted
  into the next ssh's own jump, whose tokens are expanded again, where the `%%` would be a `%`;
- **not in a `ssh://` URI**, whose own escapes differ.

Every other `%` still costs the Allow: a single `%` (`[fe80::1%en0]`, which ssh itself would
refuse), a zone outside brackets or after any other address, a zone in an earlier hop, a `%` in a
user or a port, and a `%` in `Hostname`, `User` or a jump's destination. A person can still
declare the command as a tool.
