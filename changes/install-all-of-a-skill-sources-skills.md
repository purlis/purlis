### Added

- **Install all of a skill source's skills in one review** (Skill studio preview, #4024). A
  source's page offers Install all…: every skill not installed from it yet is ticked, to untick
  and narrow, and those ticked go through one review, land at one level chosen at its top, and
  install together as one commit, all or nothing. Each skill's risky items are on its box, and
  a skill can be left out from the review itself. `purlis skill install <source> --all` does the
  same in the terminal, asking once; `--yes` skips the question. A set larger than one read of
  the source says so and offers to install it in two parts.
