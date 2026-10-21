# Amendment, 2026-10-21 (#2904): the button family is reached by its attributes alone

Every button in the window is now the button family's wrapper (ADR 0104), so the rules a
theme's parts write reach the family by `data-scope`, `data-part`, `data-variant` and React
Aria's state attributes alone. The interim selectors on the old control classes (`.ui-button`,
each variant's class and the native states), which §6 described, are gone.

- **A button names one variant**, so a variant's rule reaches only a button of that variant. A
  rule for the primary never reaches an Allow or a danger (ADR 0097 §6 and §7 here hold as
  written).
- **A size a theme sets is the family's default size, md.** Padding, gap, font size, font
  weight, height and border width never reach a button drawn at sm or lg, or one in a row that
  wraps; those keep the family's own sizes. The corners of an sm and an lg button are clamped to
  their own height.
- **The manifest is at version 2**: the button gains the `soft-accent`, `warning-soft` and `link`
  variants, none of them a consent variant.
