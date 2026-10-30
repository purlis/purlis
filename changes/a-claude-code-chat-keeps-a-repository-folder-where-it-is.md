### Changed

- **A Claude Code chat holds each repository's git folder as a whole.** A repository folder
  that was there when the chat started stays where it is, as do its git folder and the folders
  above it: the chat's commands no longer move, replace or remove them. Git works in it as
  before. Remove a branch folder with `purlis worktree remove`. A refused write of one of these
  folders says so, and offers no Allow.
