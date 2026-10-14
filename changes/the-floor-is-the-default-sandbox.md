### Changed

- **Every chat now runs at least at the floor** (#2071). In a project that has not turned the
  sandbox on, a chat has no network limits, no folder limits and no Allow asks, so it works as
  its harness does unsandboxed. It still cannot read a vault or purlis's own keys, cannot write
  what a later program runs (git hooks and config, `.claude` settings, editor tasks), and
  cannot write a folder on your `PATH` or your harness's own program; a refused install raises
  the usual block Notice. A project with `[sandbox] mode = "on"` keeps the full sandbox. Each
  chat's tab shows its level: full sandbox, floor, or no sandbox.
