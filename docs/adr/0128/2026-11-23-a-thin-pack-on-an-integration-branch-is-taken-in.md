# Amendment 2026-11-23: a thin pack on an integration branch is taken in

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8.3 (a push's
objects are read in a quarantine). Refs #7363 (item 1).

## What was wrong

The quarantine held the branch's old tip and the default branch's tip. A task branch cut from an
integration branch (`phase-1-proven`, `m75-policy`) is pushed as a thin pack whose deltas are
against that branch's objects, so `git index-pack --fix-thin` failed and the push was refused. It
failed closed, and it also made every such branch a push the person had to make.

## Decision

When git says a pack is **short of objects**, the app fetches the forge's advertised branch tips
(`ls-remote --heads` on the person's credential, deduplicated, at most `KNOWN_AT_MOST`) with their
history into the same quarantine, and takes the pack in once more (`Quarantine::take_in_known`).
The tips come from the forge, never from the chat.

- **Only a shortage of objects fetches.** The pack was short when `index-pack --fix-thin` stops
  with git's own "unresolved delta" failure (a thin pack's base is not here) or, under `--strict`,
  "did not receive expected object" (a small change with no delta misses its parent), or when `rev-list
  --objects --missing=error` fails on the tip and the same walk with `--missing=allow-any` does not
  (an object the pack left out because the forge has it is missing; one git could not read is not).
  A truncated or garbage pack, and any other `index-pack` failure, is refused at once, with no
  `ls-remote` and no fetch. A well-formed pack that names an absent base still fetches, so a chat
  can make the app fetch the forge's tips; what bounds it is the limits below, not the trigger.
- **Incremental.** The base (`open`, `also_have`) and every tip are fetched into private refs of
  the quarantine (`refs/quarantine/base/<id>`, `refs/quarantine/tips/<id>`), which no chat can
  write. Git negotiates `have` lines from refs and not from `FETCH_HEAD`, so each chunk of 256
  tips is sent only what the base and the earlier chunks do not hold, instead of the shared history
  once per chunk.
- **Bounded** (`FetchLimits::DEFAULT`): all the fetches of one run together may take 240 s (checked
  before each `git fetch`, so a run can end one fetch, at most 120 s, past it), the quarantine's
  object store may grow by at most 512 MiB beyond what the base gave it (checked after each chunk),
  and at most 4096 tips are read. A run past either bound is refused with a message that says which,
  never cut short and trusted.
- **A tip deleted between the listing and the fetch** fails its whole `git fetch`, so that chunk is
  taken again one tip at a time and the vanished tip is skipped. Nothing is let through by skipping:
  a pack that needed it is still refused by git afterwards. A run that fetched no tip at all is an
  error.

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
