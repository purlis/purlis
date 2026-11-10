### Added

- **A skill can ship an artifact template.** A skill keeps a page in its own folder as
  `artifacts/<name>/`, and its run asks for it with
  `purlis artifact pin <name> --skill <skill>` (or the `artifact_propose_pin` tool's new
  `skill`): purlis copies the page, without its data files, into the workspace's `artifacts/`
  and proposes the pin, which only you make. It is refused when it holds something shaped like a
  secret or is too large, and a copy of the same page is left as it is on the skill's next run.
  The Skill studio guide and the `purlis-skill-studio` skill say how (ADR 0122, #5974).
