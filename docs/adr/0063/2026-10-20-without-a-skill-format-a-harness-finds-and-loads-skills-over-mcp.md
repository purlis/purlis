# Amended 2026-10-20: without a skill format, a harness finds and loads skills over MCP

**2026-10-20**, ticket #2776 (SK-13), from the operator's spec #2762 (M65, Skill studio),
decisions 54 and 55. It adds a fourth route to the three above, and two tools every route can
use. The routes' carrying the plane's skills as well as the built-in ones is the amendment of
the same day for #2765.

## The decision

### 1. Two skill tools on purlis's MCP server, on every harness

purlis's MCP server gets **`skill_find`** and **`skill_load`**, beside `skill_list` and
`skill_runs`, on Claude Code, Codex, opencode and any harness that speaks MCP:

- **`skill_find`** takes a query and answers with the skills of the chat's skill set that match
  it, each by name and one line, including those the skill router hid (ADR 0102). It never
  answers with a skill the set leaves out (not granted, turned off, archived).
- **`skill_load`** takes a skill's name and answers with its `SKILL.md` and the paths of the
  other files in its folder, which the harness reads with its own tools when the skill says to.

Both follow the chat tools' rules: the chat's own place is resolved at every call, no tool takes
a workspace, a project or a path, and skills are named, never addressed by path. Both are read
or load only, so they are pre-allowed and work on a read-only project.

**Each load opens a skill run** (#2766), started by "a load through MCP". A load inside an open
run is a child run, and a second load of the skill already in charge opens nothing. This is the
one exact start that works the same on every harness; on Codex, which reports no skill use of
its own to purlis's hooks, it is the exact start the Read of a `SKILL.md` only approximates.

### 2. The fourth route: `Mcp`, for a harness with MCP and no skill format

| Harness | Route | How |
|---|---|---|
| any harness whose declaration (ADR 0073) gives MCP and no skill format | `Mcp` | purlis's MCP server is in the chat's config, as it is for every chat, and the chat's briefing carries **one line**: this chat's skills are found with `skill_find` and loaded with `skill_load` |

**One line, not a listing.** The briefing route lists every skill because the harness reads
`SKILL.md` files with its own tools; the `Mcp` route needs nothing of the harness but MCP, and
the skill comes back in the tool's answer. The line costs the same whatever the library's size,
so a skill router has nothing to trim on this route, and the index skill (ADR 0102) is
`skill_find` itself.

A harness with **neither a skill format nor MCP** gets no skills, and every skill in its chats'
skill sets says so: "not supported by this harness".

**A new harness gets skills on its first day** through this route, and moves to a native one
when its adapter learns the harness's own format. Claude Code, opencode and Codex keep their
routes, and are not also told the line: a chat is told about its skills once.

## What it cannot guarantee

- **The model has to call the tool.** With only a line in the briefing, a harness that never
  calls `skill_find` never uses a skill. Trigger tests (ADR 0101) measure this per harness, and
  the compatibility badge carries it.
- **Narrowing is not applied on this route.** A skill's `disallowed-tools` or fork can only be
  applied by a harness that reads the skill natively. The badge says "narrowing not applied", as
  it does for Codex's briefing route.
- **A harness with MCP and no file tools gets only the `SKILL.md`.** The other files are named
  by path for a harness that can read them.

## What was rejected

- **The skill's whole text in the briefing.** It would put every skill's body in every chat's
  context, which is what routing exists to prevent.
- **A `skill_load` that takes a path inside the skill folder.** The chat tools take names, not
  paths, so no tool can be pointed at a file outside the skill.
- **Writing skills into a new harness's own folders** to give it a format. ADR 0099 refuses it
  for every harness.

**Built by #2776.**
