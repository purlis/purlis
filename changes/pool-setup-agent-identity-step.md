### Added

- **Pool setup sets up the agent's identity in its own step.** The Agent identity step uses the
  same flow as the Agent identity tab, so you never leave Pool setup to give the agent its own
  GitHub identity. An identity that is already there shows as done, with what it acts as and on
  how many repos. Repos on another forge host are told plainly, with the commands and the tool
  line to add by hand, in a vault of the agent's own. A tool still to allow on this machine opens
  Settings. The step can always be skipped. Try it proves the identity with a read-only token it
  revokes at once. It doesn't yet run `gh` or `git push` in the chat itself. With no repo in the
  project, Try it says gh and git push weren't checked, and the end reads "works, except for
  this" (#4490).
