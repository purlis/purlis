# Amendment 2026-11-23: a thin pack on an integration branch is taken in

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8.3 (a push's
objects are read in a quarantine). Refs #7363 (item 1).

## What was wrong

The quarantine held the branch's old tip and the default branch's tip. A task branch cut from an
integration branch (`phase-1-proven`, `m75-policy`) is pushed as a thin pack whose deltas are
against that branch's objects, so `git index-pack --fix-thin` failed and the push was refused. It
failed closed, and it also made every such branch a push the person had to make.

## Decision

When `take_in` refuses a pack, the app fetches the forge's advertised branch tips (`ls-remote
--heads` on the person's credential, deduplicated, at most `KNOWN_AT_MOST`) with their history into
the same quarantine, and takes the pack in once more (`Quarantine::take_in_known`). The tips come
from the forge, never from the chat, and are fetched only when a pack needs them.

Kept as it was:

- **The pack is verified** by git on both attempts: every object hashed and checked, and the tip a
  commit whose objects are all here (`rev-list --objects --missing=error`).
- **The base the tree is read against does not move.** `check_push_since` still compares the tip
  with the default branch's tip (and, for a branch that is there, less its old tip). A branch cut
  from an integration branch that changes a CI file against the default branch is held, with the
  diff against the default branch's tip, however the integration branch came by the file. Reading
  against the merge base with an advertised ref would let a chat choose which advertised branch's CI
  files go unread, so it is not done; the cost is a hold, never a refusal, for a branch whose
  integration branch carries CI changes of its own.
- A forge that does not answer the listing leaves the pack refused, as before.

## Not done here

Item 4 of #7363 (a resumed chat pushing its predecessor's branch) is not in this amendment: the
only link is `Identity::resumed_from`, copied from a plane session record's `chat-id`, which a chat
can write.
