### Changed

- A chat started on a profile whose inline `--settings` add `hooks`, turn hooks off
  (`disableAllHooks`, `allowManagedHooksOnly`), allow Bash in a sandbox unasked
  (`sandbox.autoAllowBashIfSandboxed`), or set `HOME`, `CLAUDE_CONFIG_DIR` or a `PURLIS_*` or
  `CHARTER_*` variable is now treated as unattended, as a profile that switches the permission
  prompts off is, and `--safe-mode` counts as one too. A chat another chat asks for starts without
  those keys, and the refusal says what the profile does ("settings that can switch the permission
  prompts or purlis's guards off"), not only that the prompts are off. The broad `Bash` allow is
  now recognised in any case and spacing (`bash`, `Bash (*)`) (#7165).
