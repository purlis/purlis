### Security

- **Regenerate accepts a script's code only from below a workspace's root or from a skill's own
  folder.** Before, a `SKILL.md` in the plane's `workspaces/`, `personas/` or `skills/` folder, or
  a script directly under `workspaces/`, made that whole folder the code a run could read once
  approved. Any other code folder is now refused, nothing is run, and the refusal says to move
  the script into a folder of the workspace (#6120, ADR 0122 as amended 2026-11-11).
