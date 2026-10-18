# Amended 2026-10-18: a project's font files are approved with the project (purlis#2555)

The operator's ruling of 2026-10-18 amends §1 for one kind of file. A theme's colours, geometry,
icons and folders still need no approval. **Its font files do, when they come from a project's
or a workspace's theme folder.**

- **A font file is part of what a project contributes.** Each font file in a theme folder at
  the plane root or in a workspace is listed by its path in the project and the sha-256 of its
  bytes (`machine::Contribution::fonts`). The trust ask
  ([ADR 0035](../0035-a-plane-is-untrusted-until-the-operator-opens-it.md)) names them under
  their own heading, so a project that brings font files and nothing else is asked about.
- **They are listed under one budget for the whole project**: at most 64 files and 32 MiB in
  all, 24 in one theme folder's `fonts/`, and the theme folders of at most 64 workspaces, the
  project's own first. A font file past the budget is not listed, so it is never approved and
  never drawn, and the complaint names the limit.
- **A new or changed font file asks again**, as a new plugin does. A font file that is gone is
  reported and does not ask.
- **A face is drawn only as approved.** The engine reads this machine's approval of the project
  itself, and draws a face from a project's or a workspace's theme folder only when the approval
  holds its file with the digest of the bytes it read. A theme that names any other face is
  refused, and the complaint names the file.
- **The person's own theme folders need no approval**, fonts included. A folder the person
  names to `purlis theme check` has its fonts read as theirs.

§1's other rules stand: editing a theme folder asks nothing, and the window repaints on save. A
saved change to a project's font file is drawn once the project is approved again.
