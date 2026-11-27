# A pane link a program broke at the pane's edge is read on only where it plainly goes on

**Accepted 2026-11-27**: #7819 (milestone M73, Links in a pane). The steward's choice, recorded
because the rule is a guess about what a program meant, and the next change to how a pane reads
its lines should keep to it or replace it here.

## Where purlis was

The pane reads a line across the rows the terminal wrapped it over (xterm's `isWrapped`), so a
long address the terminal wrapped was already one link. A harness's full-screen interface
(Claude Code's, among others) does not let the terminal wrap: it draws each row and breaks the
line itself at the pane's width. Those rows are lines of their own to xterm, so a long address
or path was read as one piece per row. Hovering the second row found nothing, and ⌘-click on
the first opened the address cut short. A link a program marked itself (OSC 8) was read whole,
but xterm hands the link handler one row's part of it at a time, so only that row was drawn in
the link colour.

## The decision

1. **A line break may be read across only where all of these hold** (`lineAt` in
   `app/src/paneLinks.ts`):
   - the row before it fills the pane to its last column;
   - the row after it is a line of its own (not one the terminal wrapped) and its first cell is
     written, not blank;
   - the row after it does not start a link of its own: an address (`scheme://`) or a path from
     `~/`, `./` or `../`;
   - a candidate on the row before reaches its very end (after the punctuation a sentence puts
     after a link is taken off, so `…/setup.` at the edge ends there).
2. **The words joined must read as one candidate of the same kind from the same start**
   (`candidatesAcross`). Anything else is read as each row reads it alone.
3. **The joined words go to the core like any other candidate.** Nothing about what a link may
   open changes: the same resolver decides, with its asks, its file-or-issue rule for
   `owner/repo#N`, its refusals and its confinement. The pane only finds text, as before.
4. **Where the core says the joined words are no link, each row's own piece links as it did
   before the join.** A path at the edge followed by an unrelated word (`src/lib.rs` then
   `Next, …`) joins to words that name no file; the piece on its row is still asked and still
   links. An address cannot fail that way, because every `https:` address is a link, so a full
   row ending in an address followed at once by unrelated words with no space is read as one
   address. That is the cost of the rule, and it is the case this record exists to name.
5. **A marked link (OSC 8) is drawn over every row it was recorded on**, from whichever row the
   pointer is on, unless the program moved the cursor while it was open, where only the part
   xterm hands is drawn.

At most 24 rows are read as one line, as for a wrapped line, and a link that runs on past them
is no link.

## What was not done

- **Reading on across any line break.** Prose printed full-width would run into the next line,
  and a link's last word into the first word below it.
- **Following the indent a harness puts on the rows it continues** (two spaces in a Claude Code
  message). The second row then starts with a blank and nothing is joined. Reading through an
  indent would need to know which program drew the pane, and nothing decides by parsing harness
  output (CLAUDE.md).
- **Keeping the join across a resize that re-wraps the first row.** Once the pane is made
  narrower, the terminal re-wraps the row the program broke, its last part no longer reaches
  the edge, and the pieces read apart until the program draws the screen again at the new
  width, which a full-screen interface does.
