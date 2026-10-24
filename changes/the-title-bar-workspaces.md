### Changed

- **Your workspaces are in the title bar.** After the project's name, each workspace you pinned
  is a chip, with how many things wait on you there and the one you are in lit; the workspace
  you are in, when it is not pinned, is a dashed chip that pins it. **⌄** (or **⌥⌘P**,
  Ctrl+Alt+Shift+P elsewhere) opens every workspace with a filter, a pin toggle on each, the project root first and
  **New workspace…** last. **⌘1–⌘9** (Ctrl+1–9 elsewhere) go to the first nine pins, and a
  sideways swipe over the title bar steps one pin at a time. Switching slides the workspace in;
  reduced motion has none. The workspace strip under the title bar is gone (#2916).
- **With the projects sidebar hidden**, the project's name in the title bar (or ⌥⌘P) opens a
  switcher of every project and workspace. Choosing a project goes back to the workspace you were last on.
- **Themes have a `titlebar` family** of parts for the chips (`titlebar.workspace.*`,
  `titlebar.count.*`; parts manifest version 12).
- **⌘P is unchanged**: it still finds files and projects, and ⌘P ⏎ still goes back to the last
  project.
