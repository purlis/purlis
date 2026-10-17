# Amended 2026-10-17: a chat is confined on a runner as its desktop relays it

**Decided in implementation of #2217, to be confirmed** (D-2217-1 to D-2217-3; ADR 0068,
amended the same day). The ruling of 2026-10-15 stands, and this note adds to its item 2.

1. **A runner's host also takes a chat's confinement as the desktop's host relays it.** Over
   `remote-link` it accepts `confine`, `change_reach`, `reach_of` and `let_go_of_confinement`,
   and pushes `sandbox` to that link; the desktop's relay sends them on, and hands `sandbox` on,
   for a `local-ui` client of its own host alone (item 3 of the ruling).
2. **A runner composes its harness chats itself.** It refuses a start whose `launch` names a
   harness from `remote-link`: a harness chat on a runner is started by its names and composed
   there, with the sandbox the runner compiles for itself (ADR 0068, amended for #2171). The
   relayed `launch` stays for a program that names no harness.
