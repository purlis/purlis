# A skill's template is placed by its run and proposed

**2026-11-10, #5974.** Amends §1 and §9 of ADR 0122 with what building skill artifacts decided,
in the shape of [an extension's template](2026-11-09-an-extension-s-template-is-pinned-as-a-copy.md).

- **A skill ships a template in its own folder**, as `artifacts/<name>/` beside its `SKILL.md`.
  No manifest declares it: the folder is the declaration, as a skill's other files are.
- **Its run asks for it by name**: `purlis artifact pin <name> --skill <skill>`, or
  `artifact_propose_pin` with `skill`. The skill is found by its name in the chat's own skill
  set, as `skill_load` finds it, never by a path; the template is read from inside that skill's
  folder alone.
- **It is copied into the workspace's `artifacts/<name>/` without its data files**, read within
  the size cap and scanned for anything shaped like a secret before a byte is written, as an
  extension's is. Its folder keeps the template's name, not `<skill>.<name>`: the page a skill
  writes its data into is the one named in its steps.
- **In a chat the app places it, then holds the proposal** (#5963): the workspace is the app's
  record of the chat, and a chat working in a clone cannot write the plane. Placing is not
  pinning: only the person's Pin pins. A proposal refused after the copy was made removes it.
  At the person's terminal the copy is pinned at once.
- **A run again leaves a copy holding the same template as it is**, data included, where an
  extension's pin of a folder already there is refused: a skill runs again and again. A folder
  holding another template is never written over; to take a skill's newer template, unpin and
  remove the copy, then run the skill again.
- **Not yet**: the pins file does not record which skill a pin was copied from
  (`docs/plane-format.md` is unchanged), and the window's pin dialog lists no skill's templates.
