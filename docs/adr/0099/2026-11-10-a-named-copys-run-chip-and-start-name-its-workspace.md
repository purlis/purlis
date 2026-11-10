# A named copy's run chip and its start name its workspace

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-10**, ticket #6083. The amendment of the same day ([a named workspace skill on the chat's tab and in its runs](2026-11-10-a-named-workspace-skill-on-the-chats-tab-and-in-its-runs.md)) recorded a run of `workspaces/<ws>:<name>` against `<ws>`'s copy in its row. The window's run chip and the event log's `skill.started` still said only the level (`workspace`), which reads as the chat's own workspace's copy.

## What they say

- **`skill.started` says `skill_copy_workspace`** beside `skill_level`: the workspace whose copy the run is of, where its persona named another workspace's. Where the chat works in a LIVE workspace, it is what the run's row says as `copy_workspace`. A row of a chat in a LOCAL workspace says no copy at all, while the event log, which is this machine's alone, still says a LIVE copy's workspace. An exporter reads it as it reads `skill_level` (ADR 0083, amended 2026-10-23).
- **The run chip names the copy**: "Using the skill deploy of workspace ops", in its accessible name and its tooltip. Pressed, it opens the Skill studio at `ops`'s scope, since that copy's runs are counted there wherever they ran (`RunCopy::workspace_of_copy`). What the chip draws is unchanged: a pane's head has no room for the workspace's name at its narrow widths (`paneHead.ts`).
- **A LOCAL workspace's copy is said as its row says it.** The row says neither that workspace's name (SK-20) nor a level nor a tree, since `workspace` alone would read as the chat's own workspace's copy. `skill.started` leaves out all three too, and the chip says no level and opens the chat's own scope (`skilluse::CopySaid::Unsaid`).

## Worked out where the start is read, not carried on it

The start does not carry the copy. Each of the two readers works it out from what it already holds, with `skilluse::copy_said`, which reads the chat's set as `skills::named_copy_of` does for the row. It answers `CopySaid`: the chat's own copy, a named LIVE workspace's, or a LOCAL one's, which is not said.

- The event log's writer has the plane and the chat's persona as it writes `skill.started`. `skilluse::begun` works the copy out once there, for what the run ran (`tree`) and whose copy it is.
- The window's chip is told by `skillrun::SkillRuns`, which holds only the hook lines. The app hands it a way to work the copy out (`SkillRuns::name_copies_by`), reading the persona the chat was started as, which is the persona the event log is told, and the plane. It runs once for each run, outside the runs' lock, and is held while the run is open, so a tool call never reads the plane again.

**Rejected: a `copy_workspace` field on the start.** It would go on the hook's line, a typed start the window holds, `skill_load`'s line and a runner's relayed line, as the amendment before this one says. Each of them already reaches a reader that has the plane and the persona. The cost of working it out at the readers is one extra read of the chat's set, and only for a run whose persona names its skill by another workspace.
