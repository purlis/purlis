### Changed

- **`[policy]` now switches the nine families ruling Q308b added**: `guards.forge`,
  `guards.shape`, `guards.plane`, `mode`, `tools`, `trust`, `runners`, `consents` and `limits`.
  Each is enforced under `team` and `strict`, logged under `watch` and `hold`, and off under
  `open`. `trust` is the exception: every template but `open` enforces it, so a profile or
  harness declaration nobody approved still asks you first. Where you turn it off, the chat's
  tab says it started unapproved. Under `limits = "log"` a dispatch or commit limit is counted,
  not applied. An administrator's ceiling, the deepest a chain of dispatches may go (8), and a
  workspace's 0 that switches dispatch off hold in every mode. A chat another chat starts never
  runs with its profile's permission prompts off, whatever `mode` says, including a mode set in
  inline `--settings` or in a declared harness's own arguments. The plane root's
  branch-move and reset guards (A3, A3b) are now `guards.plane`'s, not `guards.git`'s, and a
  project that set `guards.git`, or a machine policy that locks it, keeps them at least as
  strict. A guard that stands behind your own ask and deny rules holds in every mode, also for a
  command too big or too deep to read, and where those rules name a program a Bash guard that
  fails to answer refuses the call rather than lets it run.
  A command too big for the guard to read is still refused while any family that reads it is
  enforced. `docs/policy-inventory-families.md` gives the family of every decision purlis makes
  about a chat, and where each family is switched.
