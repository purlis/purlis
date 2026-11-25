### Added

- **A persona or a branch folder a chat asks to remove is now one ask that lists exactly what you
  would lose.** `purlis persona remove`, `purlis worktree remove --force`, and a removal over work
  a plain removal would lose or files git ignores used to stop the chat with no way on. Asks now
  shows a row that names the thing and lists what goes, with **Remove persona**, **Remove and
  discard** or **Remove with these files**, and **Keep**. The press reads the thing again and
  removes nothing if it holds anything the row did not list, an edit to a listed file or a swap
  past the eighth entry included; where purlis could not list everything, the row offers Keep only,
  and Keep clears it. With no window open the chat is told nobody was asked, and how to have
  it asked (T1.1-67, #6956).

### Changed

- **A chat whose app is a different build is told so.** An app that cannot read a removal ask no
  longer looks like a refusal: the chat is told the app and its purlis are different builds, and
  to restart purlis or update the plugin (#6956).
