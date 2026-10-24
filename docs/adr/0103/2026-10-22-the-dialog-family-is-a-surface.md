# Amendment, 2026-10-22 (#2910): the dialog family is a surface

The dialog family (ADR 0104, `app/src/ui/dialog/`) joins the manifest at **version 7**. It is the
first family that is not a control, and the engine reads it as a surface:

- **Its parts** are the root (fill, border colour, border width, corners, shadow), the title
  (words, size, weight), the subtitle, the description and the summary (words), and the separator
  over the footer (fill). Its variants are `dialog` and `alert`, neither a consent variant.
- **No height, no padding.** A dialog's width, height, place and rows are the dialog rules',
  never a theme's, so its root takes no height and no part takes padding or a gap: such a key is
  said and left out, as any key the manifest does not have.
- **A control's rules are a control's.** The least target and the words-fit check hold the
  families whose root takes a height; a dialog's corner is not clamped to half a control's
  height. It is a surface's: drawn at most `space.6`, the inset of what it holds, and `full` is
  that, so a rounded dialog never cuts off a word or an answer in a corner.
- **The floors hold as for every family**: the title's, the subtitle's, the description's and the
  summary's words on the root's fill, at 4.5:1, each in the tier it is drawn in (the subtitle
  muted, the description secondary, the summary quiet); a part that breaks one draws the family
  default.
