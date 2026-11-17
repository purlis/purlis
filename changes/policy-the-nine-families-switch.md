### Changed

- **`[policy]` now switches the nine families ruling Q308b added**: `guards.forge`,
  `guards.shape`, `guards.plane`, `mode`, `tools`, `trust`, `runners`, `consents` and `limits`.
  Each is enforced under `team` and `strict`, logged under `watch` and `hold`, and off under
  `open`. Under `limits = "log"` a dispatch or commit limit is counted, not applied. An
  administrator's ceiling, and the deepest a chain of dispatches may go (8), hold in every mode.
  The plane root's branch-move and reset guards (A3, A3b) are now `guards.plane`'s, not
  `guards.git`'s. A guard that stands behind your own ask and deny rules holds in every mode.
  A command too big for the guard to read is still refused while any family that reads it is
  enforced. `docs/policy-inventory-families.md` gives the family of every decision purlis makes
  about a chat, and where each family is switched.
