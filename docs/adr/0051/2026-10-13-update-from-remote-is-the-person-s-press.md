# Noted (2026-10-13, qw348): Update from remote is the person's press, and follows the mode

Decided under delegation, and recorded for the operator's ruling. It amends nothing above:
"with auto-save off, incoming changes are shown and not pulled" still holds for everything
purlis does by itself.

The window gained **Update**, where the Saving view and the title bar show `↓N`. It is the
person's own press and nobody else's (`update_project` is window-only), and purlis runs it
outside every chat's sandbox: a chat that runs git in the project's root is stopped partway by
its sandbox and is refused it (ADR 0067, noted 2026-10-13).

- **What it reaches** is what the incoming loop's fetch reaches: the target branch of the
  remote recorded for the project, through the same function, with the same holds.
- **What it moves.** A clean tree that is only behind is fast-forwarded, as the incoming loop
  does. A clean tree with commits of its own has them replayed onto the fetched branch, which
  the incoming loop never does and a save's own retry does. A tree with unsaved work is never
  touched and nothing is stashed.
- **In `pr` and `pr-merge`,** the target branch still moves through its request. A request
  that merged moves the project as "A squash merge" above says. Commits an open request
  carries are not replayed: the update says they wait on it. Commits no request carries yet
  are replayed, since nothing of them is on the remote, and the next save pushes them to the
  save branch under its lease.
- **It is whole or it did not happen.** Nothing starts where a file could not be written, and
  a move git stops part-way is undone and checked. The one case that cannot be undone is said
  as such, with the files.
- **It writes no journal line.** The journal's `trigger` and `outcome` words are a format, and
  an update is not a save. Whether an update is journalled, and under which words, is open.
