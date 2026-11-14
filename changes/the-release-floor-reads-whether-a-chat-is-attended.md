### Changed

- **The release floor goes by whether purlis says a person is at a chat, not by the mode its
  harness reports.** A chat purlis started for nobody to watch (a task, a runner's chat, or one
  on a profile that switches its prompts off), or one whose Claude Code mode changed to one
  purlis didn't set, is refused a release, a tag push or a merge in every mode. A chat you
  started and watch can publish even where its harness reports skipping prompts, as Codex does
  on every line. A merge in a chat the app started is always the merge hold's, attended or not:
  you're asked in Asks, and the floor doesn't refuse it first. Whether a chat is attended is
  kept across a restart of the app, so a chat that was unattended comes back unattended. Where
  the app doesn't answer, the floor treats the chat as unattended (#6409).
