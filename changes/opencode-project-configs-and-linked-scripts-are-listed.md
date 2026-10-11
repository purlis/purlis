### Added

- **Settings lists what a project's own opencode configs load.** The harness-plugin list now
  names the npm plugins each of the project's `opencode.json`, `opencode.jsonc`,
  `.opencode/opencode.json` and `.opencode/opencode.jsonc` names, each by the file that names
  it, and names a config purlis cannot read rather than skipping it. A script linked into a
  plugin directory is listed like a file, since opencode loads it too (#1590).

