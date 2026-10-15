# Amended 2026-09-25: an extension can act and write, and purlis reports what it wrote

purlis#341 adds the `palette`, `actions` and `writes` capabilities (ADR 0053's amendment
of the same day). What this changes in the threat model:

- **A second request, from the same gate.** Running an action starts the program the same way a
  view does: the record first, the project's on or off, the fingerprint taken again over the
  whole tree at the press, one process, the deadline and the kill. The operator's press is still
  the only thing that starts it. A palette command is a press too. "One round trip per
  deliberate human action" holds.
- **Asking first is enforced by the core, not the window.** The executor refuses an action that
  asks first (its manifest's `confirm`, or `deletes`) unless the operator said yes.
- **Writes are declared, handed and watched, not confined.** The declared paths are inside the
  fingerprint and named in the prompt. Each request hands them resolved. After each question
  purlis compares what `git status` shows in the plane and reports a change outside them,
  naming the extension. This does not change decision 1: there is still no sandbox, and an
  extension can still write anywhere the operator can. What is new is that the ordinary case
  of writing outside the declared paths is visible, where before it was silent. No declared
  path may cover the files purlis reads settings, grants or vaults from.
- **What is still open:** the report cannot see an ignored path or a plane that is not a git
  repository, and it cannot tell who made a change. An extension that lies about `deletes` is
  reported, not stopped.
