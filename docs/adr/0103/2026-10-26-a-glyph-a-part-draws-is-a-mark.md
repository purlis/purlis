# Amendment, 2026-10-26 (#3099): a glyph a part draws is a mark, and a part's words are read on what is under them

§7 holds a part's words at 4.5:1 and an icon at 3:1. Two things are added.

- **A glyph a part draws is held as a mark, at 3:1.** That covers a tab's ×, the title bar's ⌄
  and a switcher row's pin, as well as an icon. Each is a glyph to be told apart, not words to
  read (WCAG 1.4.11), and is drawn in the faint tier that `check::FAINT` names for marks. Words
  a person reads stay at 4.5:1.
- **A part's words are read on what is under them.** A part that paints a ground of its own
  (the popover's key cap, the activity item's count pill, the rail's badge, the title bar's
  count) declares it as its fill. Its words are held on that fill, and a part that falls back
  paints it again. A part that paints no ground of its own but does not stand on its root's
  fill declares only the ground it stands on: a tab's count on the counts button its parent
  paints, or a switcher's row and pin on the popover they are drawn in. Its words are held on
  that ground, and no fallback paints it. A test holds each declaration to the stylesheet rule
  that draws it.
