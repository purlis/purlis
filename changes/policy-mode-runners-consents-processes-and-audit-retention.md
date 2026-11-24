### Added

- **The policy judges four more families.** `mode` refuses a task whose prompts are off
  (`mode-task-above-auto`: a task is capped at auto, and a person's own chat is never judged
  by it), `runners` holds a chat's offload to a runner and forward from one for the person's
  Approve, once and never "always" (`runners-offload`, `runners-forward`), `consents` refuses a
  chat's report or promote, which only the window's press makes (`consents-window-only`), and
  `processes` refuses `sudo`, `doas`, `pkexec`, `su`, `sudoedit` and `run0`, by program name
  whatever the path they are started by, at the floor and the full sandbox (at no sandbox the
  family is not in force), beyond the escape denies
  (`processes-elevation`; a wrapper such as `env sudo` is not read yet). Each refusal names its
  rule, level and owner (M75-36, #7045).
- **A pool's cap is a limit.** Each of a pool's twelve caps is a `runners.pool-*` key of
  `[policy.limits]`, raised only by the person's press, and counted rather than applied under
  `limits = "log"` (M75-36, #7045).

### Changed

- **Audit retention has a least and a most.** The event log keeps 7 to 90 days (30 shipped), the
  audit record at least 30 days (365 shipped, 36500 at most) and between 64 MiB (2 GiB shipped) and
  1 PiB. A number outside its range reads as the nearest end with a Notice that names the file and
  the line, an Organisation's too: an Organisation's 120 days of event log reads as 90, the most,
  never as a lock above it. Keeping less is a loosening, so it waits for the person's press and never goes below an Organisation's
  number. No policy, template or `audit = "off"` turns the record off (M75-36, #7045).
- **`context.harness_mode` is the mode the host started the chat in.** It comes from the host's
  own start record, never from a report of the chat's hook or a word of its line, and reads
  `unknown` where the host holds no record of one, which `mode-task-above-auto` allows. It reads
  the bypass for a Claude Code start by any of `--dangerously-skip-permissions`, `--yolo`,
  `--safe-mode`, `--permission-mode bypassPermissions` or inline `--settings` with a bypass
  default mode, and for a Codex start by `--dangerously-bypass-approvals-and-sandbox`,
  `--full-auto` or approval policy `never`. A project's rule on `start` reads the same mode and
  the real attendance (M75-36, #7045).
