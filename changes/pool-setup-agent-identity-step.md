### Added

- **Pool setup sets up the agent's identity in its own step.** The Agent identity step uses the
  same flow as the Agent identity tab, so you never leave Pool setup to give the agent its own
  GitHub identity. An identity that is already there shows as done, with what it acts as and on
  how many repos. Repos on another forge host are told plainly, with the token command and tool
  line to add by hand, and the step can always be skipped. With no repo in the project, Try it
  says gh and git push weren't checked, and the end reads "works, except for this" (#4490).
