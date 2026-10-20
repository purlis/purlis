# Amended 2026-10-20: without a skill format, a harness finds and loads skills over MCP

From the operator's spec #2762 (M65, Skill studio), decisions 54, 55 and 58; ticket #2776
(SK-13). This record gives each harness a route by which purlis hands a chat its skills:
`Plugin` for Claude Code, `Config` for opencode and `Briefing` for Codex. This amendment adds a
fourth route, `Mcp`, for a harness that has MCP and no skill format, and two MCP tools every
route can use.

## The decision

### 1. Two skill tools on purlis's MCP server, on every harness

purlis's MCP server gets **`skill_find`** and **`skill_load`**, beside `skill_list` and
`skill_runs`, on Claude Code, Codex, opencode and any harness that speaks MCP:

- **`skill_find`** takes a query and answers with the skills of the chat's skill set that match
  it, each by name and one line, including those a skill router hid. It never answers with a
  skill the set leaves out (not granted, turned off, archived).
- **`skill_load`** takes a skill's name and answers with its `SKILL.md` and the paths of the
  other files in its folder, which the harness reads with its own tools when the skill says to.

Both follow the chat tools' rules: the chat's own place is resolved at every call, no tool takes
a workspace, a project or a path, and skills are named, never addressed by path. Both are read
or load only, so they are pre-allowed and work on a read-only project.

**Each load opens a skill run** (#2766), started by "a load through MCP". A load inside an open
run is a child run, and a second load of the skill already in charge opens nothing. This is the
one exact start that works the same on every harness; on Codex, which reports no skill use of
its own to purlis's hooks, it is the exact start that a Read of a `SKILL.md` only approximates.

### 2. A loaded skill keeps its narrowing where the harness can apply it

A skill may only narrow a chat, never widen it (#2771). **A skill loaded through `skill_load`
keeps its narrowing on every harness that can apply it**: on Claude Code, its `disallowed-tools`
and its fork are applied to the chat as they are when Claude Code starts the skill itself.
**Where the harness cannot apply it, the load says so**: the answer to `skill_load` carries
"narrowing not applied", and the skill's compatibility badge says the same for that harness.
Codex and opencode cannot enforce a skill's narrowing (#2771), so on both a load answers with
that line; so does every harness on the `Mcp` route.

### 3. The fourth route: `Mcp`, for a harness with MCP and no skill format

| Harness | Route | How |
|---|---|---|
| any harness whose declaration (ADR 0073) gives MCP and no skill format | `Mcp` | purlis's MCP server is in the chat's config, and the chat's briefing carries **one line**: this chat's skills are found with `skill_find` and loaded with `skill_load` |

**One line, not a listing.** The `Briefing` route lists every skill because the harness reads
`SKILL.md` files with its own tools; the `Mcp` route needs nothing of the harness but MCP, and
the skill comes back in the tool's answer. The line costs the same whatever the library's size,
so a skill router has nothing to trim on this route, and `skill_find` stands in for the index
skill.

A harness with **neither a skill format nor MCP** gets no skills, and every skill in its chats'
skill sets says so: "not supported by this harness".

**A new harness gets skills on its first day** through this route, and moves to a native one
when its adapter learns the harness's own format. Claude Code, opencode and Codex keep their
routes, and are not also told the line: a chat is told about its skills once.

## What it cannot guarantee

- **The model has to call the tool.** With only a line in the briefing, a harness that never
  calls `skill_find` never uses a skill. Trigger tests measure this per harness, and the
  compatibility badge carries it.
- **A harness with MCP and no file tools gets only the `SKILL.md`.** The other files are named
  by path for a harness that can read them.

## What was rejected

- **The skill's whole text in the briefing.** It would put every skill's body in every chat's
  context, which is what routing exists to prevent.
- **A `skill_load` that takes a path inside the skill folder.** The chat tools take names, not
  paths, so no tool can be pointed at a file outside the skill.
- **Writing skills into a new harness's own folders** to give it a format. purlis never writes
  into a harness's own skills folders.
- **A silent load where narrowing cannot be applied.** The person would believe a skill narrowed
  the chat when it did not.

**Built by #2776.**
