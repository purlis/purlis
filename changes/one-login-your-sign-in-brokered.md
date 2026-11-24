### Added

- **A chat's `gh`, `glab` and `git` work on your own sign-in, with nothing to set up.** Sign in once
  to a forge in Settings › You › Accounts and trust a clone; a chat in that project then fetches,
  pulls, pushes its own branches and runs `gh` and `glab` as you, and never holds your token. Each
  chat is given a stand-in that only purlis's own proxy accepts, one proxy per run. The proxy
  gives your token only for a repo you confirmed, under the account you chose for it, and only
  for what a chat may do there: it never merges, approves, force-pushes or touches a default
  branch. A tag, a release, a fork, a merge or a push that changes CI files is asked of you in Asks
  instead; your press makes it, with your sign-in, exactly as the ask showed it. A CI-file push is
  read from the objects the chat sent in a quarantine of purlis's own, never from the chat's clone
  (ADR 0128).

### Changed

- **Approvals are refused, not asked.** A chat cannot approve its own work as you: a review that
  approves, or GitLab's approve call, is refused outright (ADR 0128).
