# Amended 2026-10-14: the link versions itself, keeps itself alive and links again

**Ruled by the operator on 2026-10-14**, in the grill on runners (spec #2041: *"a link-level
liveness ping and reconnect, and reconnecting ahead of the node's stream limits. The protocol
carries its own version, apart from the app's; hosts within one major version talk"*). Built by
#2042 (`purlis_session_protocol::hostlink`). The text of §2 and §3 is left as accepted, and this
note adds to them. What the ruling settled is items 1, 4 and 5's shape; the rest, and every
number below, was decided in #2042 and is marked *decided, to be confirmed*.

1. **The link protocol has a version of its own**, apart from the session protocol's (ADR 0068
   §4): a hello in the clear before the handshake, negotiated the way the session protocol's
   version is, opening with its own magic. **Two hosts link only within one major version**: a
   host speaks its own major and never the one before it, and a host of another major is
   refused in words that say to update purlis on the older one. Both hellos and the version
   agreed are bound into the handshake's prologue beside the answering host's device id (§3).
   A minor of either protocol moves without the other.
2. **After the handshake, the answering host says first whether it admits the link**, sealed
   like everything after: admitted, or refused and why. So a dialer whose key is not the
   runner's peer is told so, by its key's fingerprint, never the key. *Decided, to be
   confirmed.*
3. **Everything after is sealed frames of a kind**: the session protocol's bytes (at most
   16 KiB a frame, so a ping waits behind little), a ping, and a ping's answer. A kind a version
   does not know ends the link. *Decided, to be confirmed.*
4. **Liveness.** Each end pings every 10 seconds, answers each ping, and ends the link when it
   has heard nothing at all for 30 seconds. A dropped link ends with its transport; a half-open
   one ends within that deadline at both ends. *The two numbers: decided, to be confirmed.*
5. **Reconnect, and renewal ahead of a stream's limit** (§2's *"Reconnect is the desktop's"*,
   made precise). The desktop links again on its own after a link ends, with a pause that
   starts at 250 ms and doubles to 30 seconds. It also links anew every hour, ahead of any limit
   a connector puts on a stream's life (a Kubernetes node ends an exec stream after four idle
   hours by default; the pings keep it from idling, and the hour keeps it inside a hard limit
   too), opening the new link before it closes the old one. Each new link is the session
   protocol from its start: the client subscribes from its cursor and attaches its views again,
   each with a snapshot. *The pauses and the hour: decided, to be confirmed.*
6. **A build links only with a handshake it names.** The handshakes are a closed set in the
   protocol crate, and a build of the host links with §3's Noise `XX`
   (`25519_ChaChaPoly_BLAKE2s`). *Decided, to be confirmed.*
