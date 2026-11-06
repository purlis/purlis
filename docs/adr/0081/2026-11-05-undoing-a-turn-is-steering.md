# Amended 2026-11-05: undoing a turn is steering, and "No LSP" stands

The operator asked whether purlis should become a full IDE: language support by extensions,
editing everywhere. The grilling of 2026-11-05 (rulings E-Q1 to E-Q16, recorded in the
smart-ide workspace) kept §1's test and the category "the agent IDE": the light editor serves
reviewing and steering agents, and writing code by hand stays in **your editor**. Two lines of
§2 are read again here.

## Undo a turn is not a repo-wide refactor

§2 rules out *"an operation that edits more than one file at once from purlis's window"*. That
line was drawn against hand-coding operations: rename a symbol, replace across files. Putting
back every file a **Turn** changed, as it was before that turn, is the operator steering an
agent, and N3 already rules *"cross-harness checkpoints/rollback via a git ref per turn"*. So
**Undo a turn** is purlis's, under these limits:

- **Refused unless the folder is idle**, as ADR 0084 §8 defines idle, naming the chat and its
  state. A typed edit only warns (R7); undoing many files under a working agent would leave a
  branch that is half each, and nothing repairs that.
- **Only the latest turn not yet undone**, and again for the one before it. An older turn whose
  files a later turn changed would need a merge, and ADR 0084 §8 never merges. Older turns stay
  reviewable.
- **It is a Human edit**: recorded, and announced to the agent at its next turn, paths only.

**Discarding a hunk** is one file, so it is R7's edit and needs no amendment: it warns when the
folder is not idle, and is a Human edit.

## "No LSP" stands

Language intelligence while reviewing (hover, go to definition, diagnostics on an agent's diff)
was the one gap that passed §1's test. It is met without a language server: a branch's checks
(typecheck, lint, tests; ED-8, ED-9) are drawn on the diff as diagnostics, from SARIF (ADR 0114).
An extension that supplies hover and definitions as data, drawn by purlis, is reconsidered once
the review loop (RC-4, RC-7) has shipped and shows whether people still leave purlis to
understand a diff. purlis never runs a language server itself.

## Rejected

- **A full editor with language servers and an editor extension API.** The reasons in *What was
  rejected* still hold, and extension code inside the window would undo the capability model
  (ADR 0053): an extension grows purlis by capabilities, each approved, never by code in the
  window.
- **Undoing any turn, merging around later ones.** It needs the merge ADR 0084 §8 rules out.
- **Warning, not refusing, before an undo under a working agent.** See the first limit above.
