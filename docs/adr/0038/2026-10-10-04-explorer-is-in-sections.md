# Amendment, 2026-10-10: Explorer is in sections

The operator's decision of 2026-10-10 (spec #1671, B-12; built in #1677): **Explorer is in
three sections, each folding on its heading**, as an editor's explorer is.

- **Workspaces**: every workspace the strip can bring forward, in its order, with the focused
  one marked as the current item. Pressing one focuses it exactly as its tab on the strip does,
  through the same catalogue row, and its menu is the tab's. This is not a second axis: the rows
  are not a tab list and select nothing, and the strip stays the axis (ADR 0036). It is the way
  to a workspace the strip is not drawing, one press away in the side that navigates.
- **Repos and branches**: the focused workspace, its clones and their branches. A branch is one
  row; picking it still decides where the next chat starts.
- **Files**: the files of where the next chat starts. The picked branch's or repo's files are its
  first level; with the workspace itself picked, each repo's own folder is a row that opens. A
  branch's files were drawn under its row until now; each is drawn once, here.

Which sections are folded is kept in the layout file on this machine. A focused branch is still
its cockpit, which draws in place of the three sections.
