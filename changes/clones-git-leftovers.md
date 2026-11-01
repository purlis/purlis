### Security

- A filter that purlis's git runs never downloads a Git LFS file and asks none of your credential
  helpers, in any repository rather than only in a clone purlis wired. A file purlis's git checks
  out there is its pointer until `git lfs pull`, even where its object is here.
- `purlis worktree add` is refused in a repository whose own config has an `includeIf`, or an
  include in a worktree config: its checkout could read a config purlis never saw.

### Fixed

- `purlis change push` pushes the commit it showed you, by its id. A branch whose local ref went
  bad could delete the branch on the remote and report success.
- The check of a clone's Git LFS store no longer takes a `.git` folder that git itself climbs past.
