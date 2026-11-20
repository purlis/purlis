### Fixed

- **A chat on a large plane on macOS stays within auto mode's size budget** (for a plane at a
  short path). The Edit rules for the same file in each of three workspaces or more of a plane
  (`workspace.json`, a clone's `.git/config`, a manifest) are one rule on
  `<plane>/workspaces/*/<file>`. On a plane of 12 workspaces and 240 branch folders the rules auto
  mode counts fell by about 30 KB, under the 256,000-byte line where Claude Code drops git state
  and history. The glob holds that file in every workspace of the plane, those it has and those
  made later: it denies the Edit tool more, never less. A hold a person lifted in one workspace is
  denied again there for the Edit tool (the sandbox's own lists are unchanged). A plane at a
  longer path still weighs more: each character of it adds about 1.3 KB.
