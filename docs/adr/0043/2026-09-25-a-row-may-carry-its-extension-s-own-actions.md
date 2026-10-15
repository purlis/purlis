# Amendment, 2026-09-25: a row may carry its extension's own actions

purlis#341. **A row an extension's program answers may offer that extension's own
actions**, by id, when its manifest asks for the `actions` capability and declares them (ADR
0053). This answers what the previous amendment left open, *"what a view that wants to change
something would need"*, and the answer is not the `runs` grant it described:

- **`runs` is still refused** (`panel::NO_VERB`), in a manifest and in an answer. A row still
  cannot put one of purlis's verbs on the screen.
- **What a row may carry is the extension's own verb.** Pressing it asks the extension's
  program, *run action `<id>` on `<subject>`*, through the executor's gate, and the answer may
  refresh the view's blocks. It gives the extension no purlis authority. The extension already
  runs as the operator, and the action is one more question to it, started by his press.
- **The manifest decides, the answer only names.** A row names action ids. The executor refuses
  an answer that names one the manifest does not declare. The button's title, and whether purlis
  asks first, come from the manifest the operator approved.
- **Only an answer carries them.** A declared panel has no program to run an action, so a
  manifest's row that says `actions` is refused by name.
- The buttons sit beside the row's words, never inside the row's own button. Opening the row's
  card and acting on it are two different controls.
