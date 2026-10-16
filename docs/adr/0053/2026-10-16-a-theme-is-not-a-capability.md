# Amended 2026-10-16: a theme is not a capability (ADR 0098)

[ADR 0098](../0098-a-theme-folder-needs-no-approval-and-an-extensions-capabilities-do.md) rules
that a theme is data and never a capability. `contributes.themes` and `contributes.icon_themes`
need no word in `capabilities`, as before, and ADR 0053's capability vocabulary gains none for
them.

What changes is the approval of a manifest that has nothing else: one with no `capabilities`, no
program and only themes and icon themes under `contributes` is treated as theme folders, with no
fingerprint and no prompt. A manifest that names a capability or a program is approved as this
record says, and its themes are part of what is approved.

**Built by #2324.** Until then, extensions are approved as today, their themes included.
