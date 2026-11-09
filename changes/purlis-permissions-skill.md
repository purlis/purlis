### Added

- **A built-in skill for permissions: `purlis-permissions`.** Say "stop asking me about this
  command", "why was this blocked" or "allow this host", and your chat now explains what stopped
  the command: a harness prompt, purlis's sandbox or a purlis guard. It then points you to where
  purlis reads the change: a `purlis guard ask` or `allow` rule, the sandbox Notice, or your
  sandbox settings. It never sends you to edit your harness's own settings file or to lower the
  sandbox, because purlis's settings outrank those. Requests about your harness's own settings,
  such as an environment variable, a theme or the model, stay with your harness. The skill has
  trigger tests at the strict bar.
