# The routes carry the plane's skills

**2026-10-20**, ticket #2765 (SK-2), under [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md)
and the operator's spec #2762.

The three routes carry every skill in a chat's skill set, not only the built-in ones. The set is
`purlis-skills`' answer (`skill_set`); a harness adapter only turns it into argv, env and config.
Still nothing is written into a harness's config, its skills folders or the person's tree.

| Harness | Route | The plane's skills |
|---|---|---|
| Claude Code | `Plugin` | a second `--plugin-dir`: a folder purlis lays out for the set in its data folder, `<data>/skill-sets/<digest>/`, holding `.claude-plugin/plugin.json` (named `skills`) and `skills/<name>`, a link to the skill's folder in the plane. The chat calls the skill `skills:<name>`, as it calls a built-in one `purlis:<name>` |
| opencode | `Config` | the shim's `skills` option is a list: the built-in skills' directory, then `<data>/skill-sets/<digest>/skills`, each appended to `skills.paths` |
| Codex | `Briefing` | the `SessionStart` briefing lists the plane's skills with the built-in ones, each with the path of its `SKILL.md` in the plane |

## Why a folder of links

Claude Code takes a plugin per `--plugin-dir` (repeatable, measured on 2.1.304), and finds a
plugin's skills in its `skills/`, links followed: a plugin whose `skills/<name>` was a link to a
folder elsewhere listed and ran that skill (measured). opencode's scan of a `skills.paths` entry
follows links too (`symlink: true` in its glob, read from the 1.18.33 binary). So one folder of
links serves both, and every link is to the file the plane holds: an edit is never a stale copy.

**The folder is named by a digest of the set** (each skill's name and the folder its link points
to), and is complete before it is put in place under that name. Two chats with different sets
never share one, and laying out a set never changes the folder a running chat was started with.
It is rebuilt whenever it is missing, and it holds nothing that cannot be rebuilt from the plane.

**The host lays it out, never the adapter**, as it writes the git hooks: `hoststart::armed` lays
the folder out before it asks the adapter, and hands it over in the chat's `PlaneSkills`. An
adapter reads and writes nothing; it only turns the set into argv, env and config.

**It is outside every project**, in purlis's app data folder beside the git hooks the window
writes there, and held as they are (`sandbox::Applied::hold_kit`): outside every place a
sandboxed chat writes, and kept from writes past the floor's grants. A plugin folder a chat could
write is a plugin it could add hooks and MCP servers to for the next chat. The skills the links point to are
plane files a chat can write; what a skill may do is ADR 0099's and #2771's.

## What was rejected

- **`--plugin-dir <plane root>`.** A folder with no manifest loads as a plugin named after
  itself (measured), and Claude Code would also load whatever `commands/`, `agents/`,
  `hooks/hooks.json` or `.mcp.json` the plane holds: more than skills.
- **`--add-dir`.** Claude Code reads `.claude/skills` inside an added directory, a layout the
  plane's levels do not have, and an added directory is one the chat may edit.
- **Copying the skills into the folder.** A copy goes stale on the first edit.
- **The plane's `skills/` itself as opencode's path.** It would hand opencode every folder
  there, those the set leaves out (not granted, hidden by a narrower copy) included.
