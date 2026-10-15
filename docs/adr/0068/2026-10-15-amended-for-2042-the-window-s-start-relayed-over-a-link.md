# Amended for #2042, the window's start relayed over a link, 2026-10-15

**Ruled by the operator on 2026-10-15** (ADR 0078, *Ruled 2026-10-15*). §5 and the amendments
above are left as accepted, and this note adds to them.

1. **§5's `remote-link` row**, as FD-27 wrote it, gains: holding a project (`hold_project`,
   `let_go_of_project`, `deal`, `dealt_already`), a start with the window's `launch`, and the
   window's reply to a held project's hook lines (`reply_heard`), with those lines pushed to
   it. **"Relayed" is enforced by the desktop's relay alone**, which sends them on for its own
   `local-ui` client only; the runner's host cannot tell them apart and trusts its one peer, the
   desktop, as it trusts the window. An interim until the host composes a start itself (#2171).
2. **A start marked as opting out of the sandbox is `local-ui`'s alone, on every host.** It is
   refused from any other scope, `remote-link` included, before the host hears of it. What a
   relayed start runs is what the runner's one trusted peer composed (item 1).
