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
     the edge (`vim` with `nowrap`, `less -S` as it runs by default): `https://a.example` there
     and `.b.example/login` below it are two pieces of text, never one address. Only the
     alternate screen is covered so. A program that cuts its lines at the edge on the normal
     screen reads like one that broke them: `less -S` with `-X` (as `LESS=-FRSX` sets it), which
     does not switch screens, and `ps`, which cuts each line to the pane's width. Their pieces
     may join, and item 4's ask is what stands between such a join and an address opened;
   - the row before it fills the pane to its last column (a wide character's second half
     counts) with a cell that draws. What draws is one rule (`drawnAt`): a blank, a cell no
     program wrote, a concealed cell (SGR 8), a cell drawn in its own background colour (a
     palette or true colour on itself; the default colours inverted still draw), a braille
     blank (U+2800), a Hangul filler and what the core says draws nothing (`LinkCore.drawsNothing`) draw nothing, so a row padded to
     the edge with any of them stops short, as it looks. The same rule reads a row's indent,
     where its written cells end and the words of a link a program marked (OSC 8). An indent is
     counted in columns, so one of wide or combined characters (an ideographic space U+3000)
     is left out of the joined words whole;
   - the row after it is a line of its own (not one the terminal wrapped) and starts with as
     many blank cells as the hanging indent of the first row of the line before it, then a
     written cell. That indent is the first row's blank cells, and where those are followed by
     a marker and one blank, the marker and the blank as well (#7930): a paragraph a harness
     indents alike on every row (two blanks, as in a Claude Code reply after its first
     paragraph) joins, and so does one whose first row opens with a marker and hangs the rows
     after it under its words (`⏺ ` then words, the rows below indented two blanks; `- ` or
     `• ` in a list, `1. ` or `12) ` in a numbered one). A row indented more or less than that
     starts something else, and so does a row under a marker at the marker's own indent, as
     the next item of a list does. The markers are a short fixed set, `⏺ • ● ◦ ▪ ‣ - * +`, the
     bullets Claude Code, Markdown and common command-line programs put before a reply or a
     list item, and a numbered item's number, one to three digits then `.` or `)`; a first row
     opening with any other character and a blank (`→ `, `> `) hangs nothing. The rule is the cells'
     geometry alone: it names no program and reads nothing a program says about itself;
   - the row after it, past that indent, does not open as an item of its own: one character
     that is no letter or digit, or a numbered item's number, then a blank. A nested list's dash or bullet, or the `⎿`
     Claude Code puts before a tool's result under its `⏺ ` line, is the next item, and its
     marker taken on would end the link above it with a `-` or a `⎿`. The cost is a link whose
     last piece on the next row is one such character alone before a blank (`…/a_` broken
     as `…/a` at the edge and `_ then words` below), which reads as two pieces;
   - the row after it, past that indent, starts with a character a link goes on with
     (`GOES_ON_WITH`): a letter with its marks, a digit, or the punctuation an address or a path
     holds (RFC 3986's unreserved and reserved characters, and `%`). An allow-list, not a list
     of what to refuse: a frame's border or a bar (U+2500 to U+259F), a reply's `⏺`, a tool
     result's `⎿`, `●`, `•`, a prompt's `❯`, `✓`, `→` and `…` are all outside it, as is any
     marker a harness picks next. Nor does the row start a link of its own: an address
     (`scheme://`) or a path from `~/`, `./` or `../`;
   - a candidate on the row before reaches its very end (after the punctuation a sentence puts
     after a link is taken off, so `…/setup.` at the edge ends there).
2. **The words joined must read as one candidate of the same kind from the same start**
   (`candidatesAcross`). Anything else is read as each row reads it alone. The characters
   either side of the break, and every one taken on past it, are ones a link goes on with: a
   link that runs into a frame's border `│` at the edge stops there, and `more⏺words` below
   one is never taken on. A number at the edge (an issue's `#12`, `chat 6`, a path's `:4`) never takes digits
   from the next row: `3 tests pass` after `#12` is a word of its own, and `#123` would open
   another issue. An address never takes an issue's number (`#123` then a blank) as its
   fragment, nor a row that starts with `/` after its own `/`. A line is read from the first
   row of its run of breaks, so each row of a link reads the same words.
3. **The joined words go to the core like any other candidate, marked as joined.** The same
   resolver decides, with its asks, its file-or-issue rule for `owner/repo#N`, its refusals and
   its confinement. The pane only finds text, and says it read the words across a break the
   program wrote (`joined` on the candidate). A line the terminal wrapped is one line: it is
   not joined, and is not marked.
4. **A web address joined from more than one row always asks before it opens** (the person's
   ruling of 2026-11-27, #7930). The join is a guess, and a guess can change an address's host:
   a full row ending in `https://a.example` above a row starting `.b.example/login` reads as
   `https://a.example.b.example/login`, and a program that cuts its lines on the normal screen
   (item 1) or moves the cursor to the start of the next row (`CSI 2;1H`, which reads as a line
   break it wrote) makes such pairs. So the core answers a joined web address with an ask
   (`Asking::Link`, `Resolver::open`), which shows the address whole, and its hover says ⌘-click
   asks first. The ask is answered once, with its token, as every ask is. A web address on one
   row, and one the terminal wrapped, open as before. A joined file path or number does not
   ask for the join: its kind's own rules decide, and a file outside the chat's workspace asks
   as any does.
5. **Where the core says the joined words are no link, each row's own piece links as it did
   before the join.** A path at the edge followed by an unrelated word (`src/lib.rs` then
   `Next, …`) joins to words that name no file; the piece on its row is still asked and still
   links. An address cannot fail that way, because every `https:` address is a link, so a full
   row ending in an address followed at once by unrelated words with no space is read as one
   address. That is the cost of the rule, and item 4's ask is how the person sees it.
6. **A marked link (OSC 8) is drawn over every row it was recorded on**, from whichever row the
   pointer is on, while those cells still hold what they held when the program closed it.
   Where the program moved the cursor while it was open, or wrote over its cells after it
   closed (going back up to draw the screen again), only the part xterm hands is drawn: its
   other rows may now hold another link. A later link written over any of its cells counts
   whatever words it draws: the cells hold another link where its target, or the `id` the
   program gave it, differs. The same link written again over its cells is still the one link.

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
