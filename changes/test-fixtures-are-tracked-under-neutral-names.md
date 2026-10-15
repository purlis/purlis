### Changed

- **Test fixtures are tracked under neutral names.** A fixture whose name a sandboxed chat may
  not write (`.claude/settings.json`, `opencode.json`) is tracked as `<name>.fixture` and copied
  to its real name at test time, and the editor recommendation `app/.vscode/extensions.json` is
  no longer tracked, so a sandboxed chat can bring a clone of purlis up to date across any change
  to them. A test fails on a tracked path a sandboxed chat may not write (#2089).
