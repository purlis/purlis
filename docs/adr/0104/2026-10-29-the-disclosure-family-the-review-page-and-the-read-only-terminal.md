# Amendment, 2026-10-29 (#3672, #3675, #3676): the disclosure family, the review page and the terminal's read-only mode

The operator's ruling of 2026-10-28 on spec #2897: **the disclosure is a family of the closed
list**. The Skill studio's design map (#3623) needs a button that shows and hides the region under
it (Add source's More, a file to read under its risk mark, a deleted skill's last text), and four
folds already drew their own chevron (#1916). Two more pieces the map needs join the window
without a new family each.

- **The disclosure family** (`app/src/ui/disclosure/`, on React Aria's `Disclosure`,
  `DisclosureGroup` and `DisclosurePanel`): `Disclosure`, a quiet trigger of the button family
  leading with the shared twist, which turns and never swaps, then the panel it controls, in the
  reading order and named by the trigger; it starts folded unless told and the owner may hold its
  state. `DisclosureGroup` opens one at a time unless told. `Twist` is the chevron alone, for a
  fold that is not a disclosure: a tree's group of rows (`FinishedTasks`), a table's row
  (`DispatchesTab`), a menu's item (`TabChip`). A chevron swapped by hand for a fold, and
  `data-scope="disclosure"` by hand, are `purlis/disclosure-family`, an error.
- **A refused field inside a fold opens it** (`app/src/ui/reveal.ts`): the field family's form,
  in a dialog's body or on a page, sends `purlis:reveal` at the first field it refused before
  the keyboard goes there; every disclosure around it opens there and then, so the focus lands.
  The refusal is said once, by the field's own error line (#3291); the fold says nothing.
- **The review page is a member of the panel family** (`app/src/ui/review-page/`), its parts the
  `review-page` family in the manifest: a page whose body scrolls over a fixed action bar, for a
  review too long for a dialog (the install and update reviews, the grade plan). The bar's
  answers are the dialog's own answer row (`DialogAnswerRow`) and Return rule (`returnFinds`),
  so a dialog's footer and a review page's bar cannot drift. Its own folder, so it touches no line
  of the panel family's stylesheet.
- **The terminal's read-only mode is a module, not a family** (`app/src/ReadOnlyTerminal.tsx`):
  the terminal stays its own surface. It draws xterm with no cursor and no input, hides the grid
  from a screen reader and lays its text beside it as a list of lines, one Tab stop named and
  described as read-only, with find and copy from the keyboard. A saved grade chat and the theme
  specimen's terminals are drawn by it; no `role="img"` stands for a terminal's text.
