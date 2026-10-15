# Ruled 2026-10-15: `snow`, and the window's start crosses a link as its host relays it

**Ruled by the operator on 2026-10-15**, on the questions #2042 raised. §3 and §4 are left as
accepted, and this note adds to them.

1. **The handshake is §3's Noise `XX` through the `snow` crate** (licence Apache-2.0 OR MIT),
   built with only that suite's primitives (X25519, ChaCha20-Poly1305, BLAKE2s, and the
   operating system's generator). A link key's private half is never printed or logged, is
   wiped from memory when the key is dropped, and is checked against its public half when a
   kept pair is loaded. The crate's stand-in handshake stays, for tests only.
2. **A runner's host takes the window's own commands as the desktop's host relays them.** Over
   `remote-link` it accepts `hold_project` (with `let_go_of_project`, `deal` and
   `dealt_already`, which go with it), a start carrying the window's `launch`, and `reply_heard`
   (the window's reply to a line a chat's hooks wrote, which can carry an ask's answer), and
   pushes a held project's hook lines (`heard`) to that link. It is an interim, until the host
   composes a start itself (#2171), and amends ADR 0068 §5's `remote-link` row (ADR 0068,
   amended the same day).
3. **What "relayed" means, and who enforces it.** The desktop's relay sends these on for a
   `local-ui` client of its own host alone; that is the only place the rule is enforced. A
   runner cannot tell a relayed command from one its peer composed, so **a runner trusts its
   peer as it trusts the window**: to compose the programs it starts and the environment they
   start with, and to answer its chats' asks. That trust is why a runner's peer is one device:
   **a runner holds one peer, the desktop that relays to it**, where §3 allowed one per device
   of its owner. More than one waits for a ruling of its own.
4. **A start marked as opting out of the sandbox is the window's alone, on every host.** A host
   refuses a start marked as opting out from any scope but its own `local-ui`, `remote-link`
   included (§4: an opt-out is the window's alone, and a move asks it again on the new device).
   What a relayed start runs, sandbox included, is what the runner's one trusted peer composed
   (item 3).
