### Added

- **The policy judges four more families.** `mode` refuses a task whose prompts are off
  (`mode-task-above-auto`: a task is capped at auto, and a person's own chat is never judged
  by it), `runners` holds a chat's offload to a runner and forward from one for the person's
  Approve, once and never "always" (`runners-offload`, `runners-forward`), `consents` refuses a
  chat's report or promote, which only the window's press makes (`consents-window-only`), and
  `processes` refuses `sudo`, `doas` and `pkexec` at the floor and the full sandbox, beyond the
  escape denies (`processes-elevation`). Each refusal names its rule, level and owner (M75-36,
  #7045).
- **A pool's cap is a limit.** Each of a pool's twelve caps is a `runners.pool-*` key of
  `[policy.limits]`, raised only by the person's press, and counted rather than applied under
  `limits = "log"` (M75-36, #7045).

### Changed

- **Audit retention has a least and a most.** The event log keeps 7 to 90 days (30 shipped), the
  audit record at least 30 days (365 shipped) and at least 64 MiB (2 GiB shipped). A number outside
  its range reads as the nearest end with a Notice that names the file and the line, and keeping
  less is a loosening, so it waits for the person's press and never goes below an Organisation's
  number. No policy, template or `audit = "off"` turns the record off (M75-36, #7045).
- **`context.harness_mode` is the mode the host started the chat in.** It comes from the host's
  own start record, never from a report of the chat's hook or a word of its line, and reads
  `unknown` where the host holds no record of one (M75-36, #7045).
