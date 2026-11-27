# Amended 2026-11-20: a runner holds no forge sign-in

Proposed by the steward on 2026-11-20 as slice H of the one-login train, from ADR 0128 (D9 and the
amendment list).

## What changes

- **V9.** Was: *"PRs are opened from the laptop, so the runner needs no forge credential (FI3 by
  construction)"*. Now: *"A runner holds no forge sign-in. With the desktop present, its forge
  calls are forwarded to the desktop's proxy; headless, it uses a token connection the operator
  provisions (D9). PRs are opened by the app."*
- **No installation token in a runner's environment.** A runner chat is no longer handed a
  `GH_TOKEN` an App's installation token fills.
