### Changed

- **The Chats list keeps its folds across a relaunch.** A chat you folded or opened in the Chats
  view comes back that way after purlis restarts, kept with the project's other views in
  `layout.json` and keyed by the chat's id, which it keeps across launches (#1687).
- **The Changes icon counts each branch's uncommitted files.** A branch folder's changed and
  untracked files are counted with its clone's (#1701, #1718).
- **The Personas view lists the project's personas at the project root**, where it said there
  was no workspace focused (#1686).
