# Ruled (V78, 2026-10-03, SD-2 slice 4)

The four questions SD-2's last slice (#1056) left open:

1. **No new CLI word.** One chat opts out from the window's new-chat picker, "Start without the
   sandbox", with the reason the sandbox cannot be applied shown beside it. §7's "or from
   `purlis` on a human client scope" is not built: the picker is the one place an opt-out is
   made. A project's default stays `[sandbox] mode` in `charter.toml`.
2. **Windows starts are audited.** Every start without the sandbox writes `trust.sandbox.off`,
   the forced Windows ones (V21 3) included. For those, the actor is `purlis (no backend on
   this OS)`, a host actor (ADR 0075 §2), never the operator.
3. **SD-30's install action types the distribution's install command into a shell tab at the
   project root, and does not run it.** Installing needs `sudo`. This is unlike FR-29's
   installers (V65), which one press runs.
4. **The opt-out rate is a local count.** `purlis doctor` and Project settings show it, and it
   is never sent anywhere.

The offer to an existing project (§1, V21 1) is a notice in the project view, answered once
either way, never a dialog, so it costs nothing of the first run's interrupt budget.
