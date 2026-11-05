### Added

- **Install from a terminal.** `curl -fsSL https://raw.githubusercontent.com/purlis/purlis/main/install.sh | sh`
  installs the latest stable release on macOS (Apple silicon) or Linux (x86_64), and
  `PURLIS_CHANNEL=dev` installs the dev build. On macOS it avoids the first-launch refusal an
  ad-hoc signed app downloaded in a browser meets. When `gh` is signed in, it checks each
  download's build provenance first and installs nothing that fails.
