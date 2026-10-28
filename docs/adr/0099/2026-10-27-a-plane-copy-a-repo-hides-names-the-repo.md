# A plane copy a repo hides names the repo, as #3234 built it

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-27**, ticket #3461 (after #3436, SK-46 #3149 and #3234), spec #2762 decision 51. It corrects one quotation in the [2026-10-21 amendment](2026-10-21-harness-and-repo-skills-as-built.md), and records what `--json` says of the copy a repo hides.

## What was wrong

- The 2026-10-21 amendment quotes the left-out line of a plane copy a repo's copy takes the name of as "hidden by the repo skill of this name". Since #3234 the line names the clone: "hidden by the skill of this name in the repo <name>" (`Reason::HiddenByRepo`). The Skill studio, `purlis skill list` and `show`, and the chat's tab all say it so.

## What holds now

- **On opencode and Codex** a chat in a clone is handed the repo's copy of a name, and the plane's copy is left out of that chat's set, "hidden by the skill of this name in the repo <name>". The rest of that paragraph of the 2026-10-21 amendment holds as written.
- **`purlis skill list --json` and `show --json` give that left-out copy its `clashes`** (#3461), as they give the copy purlis hands: the repo's copy that won on that program, with `outcome` `repo-wins` and the repo as its `owner` (`HarnessInTheStudio::clashes_of_left_out`). A reader of either has whose copy won without reading the left-out line. A copy left out because a clone's skills folder could not be read whole has none.
