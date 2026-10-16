# Amended 2026-10-16: a theme folder needs no approval (ADR 0098)

[ADR 0098](../0098-a-theme-folder-needs-no-approval-and-an-extensions-capabilities-do.md) amends
decision 3 for themes. Decision 3's trust shape (show what it contributes, ask once, remember the
fingerprint, ask again when it changes) is for what runs:

- **A theme folder needs no approval**: the person's own, a project's and a workspace's. A theme
  is data against a closed, bounded grammar, and reading one grants nothing.
- **An extension with a capability or a program is approved as before**, its fingerprint over its
  whole folder (the amendment of 2026-09-22), the themes in it included.
- **An extension that asks for no capability, names no program and contributes only themes and
  icon themes is treated as theme folders**: no fingerprint, no prompt, and editing its themes
  asks nothing. It is still installed by path, by the person, as decision 3 says.
