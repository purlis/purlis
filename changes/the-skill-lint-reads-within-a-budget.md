### Changed

- **The skill lint reads at most 1 MiB of a skill's other files, and a studio list at most 16 MiB
  for the skills from outside the project and 16 MiB for the project's own.** A skill whose scripts and notes go past either says how many of its
  files were not read for the lint, so a clone full of large skills no longer makes the Skill
  studio or `purlis skill list` read tens of megabytes (#3459).
- **The files beside your own skills are read the way their `SKILL.md` is.** A script or Codex's
  `agents/openai.yaml` that is a hard link (the Nix store, `cp -al`, `rsync --link-dest`) is now
  linted and read in your own skill folders, as its `SKILL.md` already was. A repo's or the
  project's is still refused (#3459).
