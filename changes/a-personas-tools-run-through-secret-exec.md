### Added

- **A persona's tools run through `purlis secret exec` by their plain name.** The project's
  `[sandbox.personas.<persona>] tools = { kubectl = { vault, as, key } }` makes a plain
  `kubectl` in a sandboxed chat running as that persona run through the brokered
  `purlis secret exec`, with the credential handed as a private file or a variable, so the
  chat never reads the vault. The real program is found where no chat writes, and a shell, an
  interpreter, `env`, `sudo`, `xargs` or purlis itself is refused as a tool. A persona's tools
  work on a machine only once the person there allows them, with the persona's hosts, on the
  same Notice. An administrator's policy can forbid persona tools with
  `"persona-tools": false` (#1407, #1362).

### Changed

- **A persona host or tool that policy locks out is refused when the project file is saved**,
  as a project host already was, named with its persona (#1407).
- **A harness profile named by a `[dispatch.profiles]` list links to Settings › Dispatch**
  when a rename or remove is refused over it (#1522).
