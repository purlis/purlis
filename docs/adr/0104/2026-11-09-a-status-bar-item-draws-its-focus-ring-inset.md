# Amendment, 2026-11-09 (#1922): a status bar item draws its focus ring inside itself

The amendment of 2026-10-22 put a row's 2 px focus ring inside it, because a list's rows sit
edge to edge and a scrolling list clips what is outside them; the token that carries it,
`--focus-ring-inset`, was already the ring of a tab, for the same reason, in a strip. **A status
item of the status bar (`variant="bar"`) is ringed the same way**, inside its own edge. Every other
control, the title bar's status pills included, keeps its ring outside it.

- **Why:** the status bar is a strip that clips: its start gives way in a narrow window and cuts
  what leaves it. With the ring outside, the bar had to keep `--space-2` empty above and below
  its items as the ring's room, so it stood about 30 px tall where its items are 24 px. With the
  ring inside, the bar is `--size-chrome` alone.
- **What it changes:** `ui/status/status.css` rings a `bar` item at `--focus-ring-inset` and the
  bar has no padding above or below. The ring still shows from the keyboard only, at 2 px in
  `focus.ring`, held to 3:1 on the bar's ground, and changes no size. The Notices item's dot
  sits on the bar's top edge, which the bar clips along its length only.
- **How it was decided:** #1922 asked for this variant and named it a design-system call. It
  follows the rule the token already states (a strip never clips a ring) and is recorded here for
  the operator to confirm or turn back on review of the pull request.
