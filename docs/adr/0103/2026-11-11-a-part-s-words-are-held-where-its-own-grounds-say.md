# Amendment, 2026-11-11 (#6175): a part's words are held where its own grounds say

The amendment of the same day for #6163 has `state_words_tests.rs` read a family's root, with no
fill of its own, on the default fills of the surfaces `STANDS_ON` places it on. It read a family's
other parts only when they named words of their own, and only on their own grounds.

- **A part's words are read as the parts check draws them.** That is the part's own words, else
  its variant's (`VARIANT_WORDS`), else the root's. A part that draws no words, or that the check
  holds as a mark (`is_mark`: an icon, a Notice's mark), is not read as words. Before, a trouble
  Notice's message and a danger button's label were skipped because they take the root's danger
  colour. Each lies on its root's fill, so the pairs already held them.
- **A part with no fill of its own is read where its root stands.** That is its grounds
  (`Family::grounds_of`), and each default fill of the surfaces its family stands on.
- **A part that names grounds of its own (`Family::under`) is read on such a fill only where
  those grounds name it.** This is how the parts check reads it there (`under_part`). A field
  stands in a list's and a table's rows. A field's error does not: only a field that can refuse
  keeps its error's line, and none stands in a row (`FIELD_ERROR_GROUNDS`, held to the components
  by `refusingFields.guard.test.ts`). So `danger.text` is **not** held at the text floor on
  `list.selected`, `surface.hover` or `attention.surface` for a field's error. Holding it there
  would refuse a theme over a pairing the window never paints.

Measured first, in case this ever changes: every shipped appearance already clears those three
pairs, at 7.39:1 at the least (Purlis Classic Dark, on a chosen row). If a field that can refuse
ever stands in a row, `refusingFields.guard.test.ts` fails until `FIELD_ERROR_GROUNDS` names
that ground, and the guard then demands the pair.

No shipped colour moved, and no new pair was needed.
