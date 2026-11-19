### Fixed

- **A chat on a large plane on macOS stays within auto mode's size budget.** The Edit rules for
  the same file in each of three workspaces or more (`workspace.json`, a clone's
  `.git/config`, a manifest) are one rule on `<plane>/workspaces/*/<file>`, which holds each
  workspace's and the same file in a workspace made later. On a plane of 12 workspaces and 240
  branch folders the rules auto mode counts fell by about 30 KB, back under the 256,000-byte
  line where Claude Code drops git state and history. No path is denied that was not, in a
  workspace the plane has, and none is let go.
