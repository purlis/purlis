### Fixed

- **Allowing certificate checks once is enough, and Settings says whether they are allowed.**
  A project's `certificate-checks = true` says the project expects them and allows nothing by
  itself, so `gh` and other Go tools failed with `x509: OSStatus -26276` in every sandboxed chat
  although the switch in Settings was on. Now:
  - the Notice's main button is **Allow for me on this machine**, so one press covers every
    chat of the project from then on (it used to be Allow once, which asked again in each chat);
  - Settings › Sandbox has a row **Certificate checks on this machine** that says whether they
    are allowed here, with **Allow on this machine**;
  - a chat that started before you allowed them says so on its block, with **Restart this
    chat**, where it used to ask for the same Allow again;
  - a chat that is mid-turn when you allow something says that it restarts once its turn ends,
    with **Restart now**, where it used to show nothing until the turn ended.

  If you set `certificate-checks = true` in a project: open Settings › Sandbox there once and
  press Allow on this machine, or answer the next Notice with its main button.
