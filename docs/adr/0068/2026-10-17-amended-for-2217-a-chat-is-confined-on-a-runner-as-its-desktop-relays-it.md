# Amended for #2217, a chat is confined on a runner as its desktop relays it, 2026-10-17

Spec #2041 (ruled 2026-10-14) asks that a runner chat be sandboxed exactly as a local one, and
#2217 decides what of a chat's confinement crosses the link between hosts. §5 and the
amendments above are left as accepted, and this note adds to them; the operator's rulings of
2026-10-15 on the window's commands relayed over a link (the amendment for #2042) stand as
ruled, and this note extends their "relayed" column by one row. Every item is **decided in
implementation, to be confirmed** (D-2217-1 to D-2217-5).

1. **A chat's confinement is relayed as the window's other commands are.** Over `remote-link`
   a runner's host takes `confine`, `change_reach`, `reach_of` and `let_go_of_confinement`, and
   pushes `sandbox` to that link. It starts the confinement with its own binary's proxy, from
   the plan its peer sent, and holds it as its own host holds the window's: it lives as long as
   the program in it, its proxy asks only while a program runs in it and a link that may confine
   is there, and its asks are answered and its ports withdrawn by that link. FD-27's
   confinement row becomes `local-ui`, and `remote-link` relayed. *D-2217-1.*
2. **"Relayed" is still enforced by the desktop's relay alone.** The relay sends the four
   commands on for its own `local-ui` client only, and hands what only the window hears (a held
   project's hook lines, how saving it goes, what a chat's sandbox did) to a `local-ui` client
   only, whatever the runner pushes to the relay's link. So no client of the desktop's host but
   the window confines a chat on a runner or hears its proxy, as on the desktop's own host.
   *D-2217-2.*
3. **A runner composes every harness chat it runs.** A start whose `launch` names a harness is a
   chat the window composed with the sandbox it compiled for its own machine; a host takes one
   from its own `local-ui` link alone, and refuses one from every other scope, `remote-link`
   included, before its host hears of it. A harness chat on a runner is started by its names
   (the amendment for #2171), so its sandbox is the one the runner compiles for itself and its
   confinement the one the runner plans. The relayed `launch` stays for a program that names no
   harness, such as a shell. FD-27's table gains the row, `local-ui` only. *D-2217-3.*
4. **`purlis serve` starts a chat's confinement**, with the same core function the window's own
   host starts one with (`purlis_core::sandbox::hosted::start`), so a runner's host composes a
   sandboxed chat and confines it with its own proxy. *D-2217-4.*
5. **No wire change.** The session protocol already carries every command, answer and notice
   above, so it stays at its version; only who may send and hear them changes. *D-2217-5.*
