# Amendment, 2026-10-04 (V89j): a sixth piece, SettingActions

The operator's ruling V89j adds **SettingActions** to the settings set: the row of buttons a
form ends in (Save and Cancel, Use this, Move, Restore, a dialog's Create and Cancel). Every
form ends in one, and the five pieces had none: the Saving view, a memory's editor, workspace
repos and Updates drew theirs with the old `settings-actions` class, and the dialogs with
`doing`. So the old classes could never be deleted without it (DS-3e, #1177).

It is held to the same rules as the other five:

- **A row and nothing more.** The buttons are native `<button>`s the call site writes, with
  their own `type`, `disabled` and `onClick`, so the next person still sees what each one is.
  It adds no props of its own; it draws the buttons alike, from the same tokens as a row's
  reset, and marks a destroying one by the window's existing `ends-it`.
- **A dialog's answer bar is not a form's buttons.** A question that ends in Cancel and one
  act, with nothing to fill in (the quit warning, a delete's confirm), keeps its own answer row;
  SettingActions ends a form drawn from the set.
- **Six pieces now, and a seventh is a new amendment to this record.**

DS-3e also deleted the old hand-built classes (`settings-*`, `asks`, `choices`, `choice`, `who`,
`picking`) once nothing used them, and `app/src/settings/oldFormClasses.test.ts` fails if one
comes back.
