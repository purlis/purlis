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

1. **A line break may be read across only where all of these hold** (`lineAt` and
   `goesOnAfter` in `app/src/paneLinks.ts`):
   - the pane shows its normal screen. On the alternate screen (an editor, a pager, a
     multiplexer) a row is a picture the program drew, and one that does not wrap cuts a line at
     the edge (`vim` with `nowrap`, `less -S`): `https://bank.example` there and
     `.evil.example/login` below it are two pieces of text, never one address;
   - the row before it fills the pane to its last column (a wide character's second half
     counts);
   - the row after it is a line of its own (not one the terminal wrapped) and starts with as
     many blank cells as the first row of the line before it, then a written cell. A harness
     that indents the rows it goes on with (two spaces in a Claude Code message) indents its
     first row's words the same, so the rows line up; a row indented more or less than that
     starts something else. The rule is the cells' geometry alone: it names no program and
     reads nothing a program says about itself;
   - the row after it, past that indent, does not start with a box-drawing or block character
     (U+2500 to U+259F), the border of a frame or a bar, and does not start a link of its own:
     an address (`scheme://`) or a path from `~/`, `./` or `../`;
   - a candidate on the row before reaches its very end (after the punctuation a sentence puts
     after a link is taken off, so `…/setup.` at the edge ends there).
2. **The words joined must read as one candidate of the same kind from the same start**
   (`candidatesAcross`). Anything else is read as each row reads it alone. They never take in a
   box-drawing or block character: a link that runs into a frame's border `│` at the edge stops
   there. A number at the edge (an issue's `#12`, `chat 6`, a path's `:4`) never takes digits
   from the next row: `3 tests pass` after `#12` is a word of its own, and `#123` would open
   another issue. An address never takes an issue's number (`#123` then a blank) as its
   fragment, nor a row that starts with `/` after its own `/`. A line is read from the first
   row of its run of breaks, so each row of a link reads the same words.
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
is no link. A marked link longer than that is drawn whole from its first 24 rows, and as the
part xterm hands from a row past them.

## What was not done

- **Reading on across any line break.** Prose printed full-width would run into the next line,
  and a link's last word into the first word below it.
- **Reading through any indent.** A row indented further than the line's first row is as
  often a list item, a quote or a code block under it as a link going on; only an equal indent
  joins.
- **Asking which program drew the pane** to pick its rule (Claude Code's indent, another
  harness's frame). Nothing decides by parsing harness output (CLAUDE.md), and the geometry
  above already covers the indent.
- **Keeping the join across a resize that re-wraps the first row.** Once the pane is made
  narrower, the terminal re-wraps the row the program broke, its last part no longer reaches
  the edge, and the pieces read apart until the program draws the screen again at the new
  width, which a full-screen interface does.
