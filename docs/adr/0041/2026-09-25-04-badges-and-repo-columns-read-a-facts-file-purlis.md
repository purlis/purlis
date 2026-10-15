# Amended 2026-09-25: badges and repo columns read a facts file (purlis#340)

The `badges` and `repo-columns` capabilities (ADR 0053) let an extension show values in the
status bar, the terminal footer and the repo table. purlis reads them from `<state>/facts.json`
and never starts the program to draw them, so this adds no process and changes nothing in
decision 2's table. What it adds to the threat model:

- **The facts file is extension-written input on a hot path.** `charter statusline` reads it on
  every turn. It is opened without following links, refused over 64 KiB, and parsed as data;
  each value is bounded and refused if it holds anything that draws as nothing. A bad file
  contributes nothing and says why. It never fails the footer.
- **It fills only what the manifest declared.** The badges and columns are inside the
  manifest's bytes, so they are fingerprinted and named in the approval prompt. A field the
  manifest did not declare contributes nothing and is reported, so a facts file cannot put
  anything on screen that the operator was not asked about.
- **Approval is re-checked before anything is drawn.** The reader takes the executor's gate: the
  record, then the fingerprint over the whole tree. An extension that changed since it was
  approved contributes nothing, and the project and workspace on/off (ADR 0048) is asked as well.
- **What it does not close.** The state directory is outside the fingerprint (purlis#152),
  so the values can change at any time without asking. That is the point: they are data the
  extension reports, not code, and they are drawn as text beside its name.
