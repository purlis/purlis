### Fixed

- **Connect reaches Docker on another machine over SSH on macOS again.** purlis keeps its data
  under `~/Library/Application Support`, and ssh took the space in that path for the end of the
  file it records the machine's key in, so Connect said SSH didn't answer when it did. Where ssh
  answers and still leaves no key, Connect now says so, with what ssh said, and marks the key's
  line as the one that failed, not the machine's answer.
