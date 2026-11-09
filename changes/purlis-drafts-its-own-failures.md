### Added

- **purlis drafts a report of its own failures in a chat.** With Agent drafts allowed, a
  `purlis` command that fails inside purlis (it now exits 70 for an internal failure, never for
  a refusal or a usage error), a purlis tool that failed inside purlis, and a sandbox block of
  purlis's own operation each become a Report draft in the Inbox, with the call, its exit and
  its scrubbed error. The chat is told once, on its next turn, and asked to add what it was
  doing with `purlis report bug --adds <draft>` (#5697).
