# Amendment, 2026-09-26: what the save indicator may take, and the narrowest window

The bar's right-hand end is described above as ✋, About and the updater. It also holds the save
indicator (ADR 0051, purlis#294), and that indicator was the one thing on the end that
could starve the project tabs. A blocked stage is a whole sentence: "Blocked: this plane is not
a git repository, so there is nothing to commit to" is 437 px wide, and in a 1024 px window it
left room for one project tab. charter#400 capped the words at `12rem` with an ellipsis. The operator
ruled on 2026-09-26 how far that goes (charter#403):

- **The words keep their `12rem` cap.** A longer stage is cut with an ellipsis. It is read in
  full from the button's name and `title`, and in the Saving view the button opens.
- **What came in is its own `↓N`, and it is never cut.** ADR 0051 wants the incoming count
  seen. While it was the end of the capped text (" · 12 incoming"), the ellipsis cut it before
  anything else. It is now an element after the words that does not shrink. The button's name
  and `title` still say "N incoming" in full, so the bar's `↓N` is the short form of the
  glossary's **Incoming** and never the only place it is said. It is the text the operator
  ruled, not a Lucide arrow: it is a count with a direction, read as one token.
- **1024 px is the narrowest window purlis supports.** At that width the bar keeps room for
  two project tabs, whatever the indicator says (`title-bar.e2e.ts`). The window's `minWidth`
  in `tauri.conf.json` is 1024, so it cannot be made narrower than the width this is promised
  at.

So the save indicator may take at most its mark, `12rem` of words, a `↓N` and its save button.
The tabs give way before any of those, as they do before the rest of the end.
