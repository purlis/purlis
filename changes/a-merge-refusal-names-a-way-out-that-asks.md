### Fixed

- **A merge refusal's way out leads to an ask.** Where the app and the chat's purlis are
  different builds, a held merge says so and asks the person to restart purlis or update the
  plugin, and names no command that would be refused again. A merge's `--help` or `-h` runs, in an unattended chat too, where it stands
  only beside plain words and the command's boolean flags, each spelled on its own; a flag
  cluster such as `-st -h`, which merges with `-h` as its subject, is held. Each
  refusal says that a yes in the chat merges nothing, and only the person's press of Merge in
  Asks does. purlis's own read of a GitLab merge request gives glab its current prompt switch,
  reads past a notice glab prints before its JSON, and says glab's version and what came first
  when the answer is not JSON (#6960).
