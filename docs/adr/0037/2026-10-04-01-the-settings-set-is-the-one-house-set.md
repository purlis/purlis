# Amendment, 2026-10-04 (V89f): the settings set is the one house set

The operator's settings grill (rulings V89a–i, the Settings spec on #558) amends the "house
abstraction layer" refusal above with **one named exception**, and only one: the settings set in
`app/src/settings/components.tsx` — **SettingsLayout**, **SettingGroup**, **SettingRow**,
**Field** and **Choice**. It is DS-3's (#626) *expand* step, and SE-16 (#1166) adds it.

**Why here, and why only this.**

- **The refusal did not stop the drift it was meant to stop. It moved the drift into CSS.**
  Two hand-built form styles exist: `settings-field` / `settings-hint` on Project settings,
  Saving, Memory and workspace repos, and `choices` / `choice` in the dialogs (New project, New
  vault, Start chat, First run). Each draws the same row, a label with a control and a line of
  help, with its own spacing, focus rules and names. Every new screen copied one of the two.
- **The screens move onto it as an expand–contract change.** The set is added beside the old
  classes, each screen moves in a ticket of its own, and the old `settings-*`, `choices` and
  `choice` classes are deleted last, once nothing uses them.

**What keeps it from being the layer this record refuses.**

- **Five pieces, and a sixth is a new amendment to this record.** It is not a commit.
- **The call-site test above still holds.** Each piece is a thin layer over a native element or
  a Radix primitive already in the window: radio group, checkbox, roving focus. Its props are
  the primitive's own words (`value`, `onValueChange`, `checked`, `onCheckedChange`), so the
  next person can see which primitive it is and reach its props.
- **The look is still purlis's.** The set is drawn in `App.css` under `ui-*` classes, and every
  colour in them is a design-system token.

`<ConfirmModal open onConfirm>` and every other house wrapper are still refused. This amendment
names one exception, and the refusal stays as it is for everything else. `AGENTS.md`,
`docs/ui-primitives.md` and `docs/design-system.md` name the same exception.
