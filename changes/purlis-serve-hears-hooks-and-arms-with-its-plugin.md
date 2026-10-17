### Added

- **A sandboxed Claude Code chat starts on a runner.** `purlis serve` arms each chat it starts
  with purlis's plugin, found beside its binary, and purlis's git hooks, and hears each held
  project's hooks on the same channel, with the same checks, as the app's own session host. The
  runner image ships the plugin at `/usr/local/lib/purlis/plugin`, and the runner image
  contract asks for it (#2447).
