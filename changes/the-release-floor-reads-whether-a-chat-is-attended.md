### Changed

- **The release floor goes by whether purlis says a person is at a chat, not by the mode its
  harness reports.** A chat purlis started for nobody to watch (a task, a runner's chat, or one
  on a profile that switches its prompts off), or one whose Claude Code mode changed to one
  purlis didn't set, is refused a release, a tag push or a merge in every mode. In a Claude
  Code chat you watch, a line that itself runs with prompts off is still refused. A Codex chat
  you watch can publish: Codex reports prompts off on every line, so that report says nothing
  about whether you're there, and a Codex chat nobody watches is still refused. A
  merge in a chat the app started is always the merge hold's, attended or not: you're asked in
  Asks, and the floor doesn't refuse it first. Whether a chat is attended is kept across a
  restart of the app, a Restart, a Restart without sandbox and a Resume after End, so a chat
  that was unattended comes back unattended. Where the app doesn't answer, the floor treats the
  chat as unattended (#6409).
