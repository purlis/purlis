# A load that would lose its narrowing is refused and sent to the harness's own start

**2026-10-21**, ticket #2776 (SK-13), amending §1 and §2 of
[without a skill format, a harness finds and loads skills over MCP](2026-10-20-without-a-skill-format-a-harness-finds-and-loads-skills-over-mcp.md),
under the operator's spec #2762. It also records where the route's rule and the one line live.

## §2 amended: on Claude Code, a narrowing skill is not loaded

The 2026-10-20 record says a skill loaded through `skill_load` keeps its narrowing on Claude Code,
"its `disallowed-tools` and its fork applied to the chat". A load cannot do that: it hands the
chat the skill's text, and text pasted into the chat takes no tool away and starts no fork. Only
Claude Code's own start of the skill applies what the skill narrows.

So **on a harness whose own start applies a skill's narrowing, `skill_load` of a skill that
narrows the chat (`disallowed-tools`, or `context: fork`) is refused**. It loads nothing and
opens no run. Its answer names what the skill narrows and tells the chat to start the skill with
the harness's own skill tool, by the name purlis armed it with: `skills:<name>` for the plane's,
`purlis:<name>` for a built-in one (`HarnessAdapter::narrowed_start`). That start applies the
narrowing and opens the run, started by the Skill tool.

**This holds only for a chat purlis armed**, one whose harness it names (`PURLIS_HARNESS`). A
terminal's own Claude Code names none, and has no `skills` plugin to start a plane skill from. There
the skill is loaded, and the answer says "Narrowing not applied", as on Codex, opencode and the
`Mcp` route.

A skill that narrows nothing loads the same way on every harness.

## §1 amended: which load opens a run

"Each load opens a skill run" now reads: **each load that hands the chat a skill opens a skill
run.** The refused load of §2 opens none; the start it sends the chat to opens one. The run opens
through the one tracker every start goes through. The server sends the host the line a tool hook
sends, a `ToolCall` with the word `skill-load` and the run it starts (`started_by` `mcp-load`).
It is recorded as `hook.skill-load` just after the run's `skill.started`. Where no host hears it,
the run's start is logged in the plane's skill log, as a hook's unheard start is.

**A run records which copy of the skill it is of**: its level and the workspace the chat worked
in. `skill_runs` shows the runs of the chat's own copies, in its own place, alone. A row that
does not say, written before this, is no copy's and is not shown there.

## Where the route's rule lives

A harness purlis ships keeps its route. A chat that names no harness is a terminal's own Claude
Code (`Harness::of_chat`, the one place that says so). A harness the project or this machine
declares (ADR 0073) takes `Mcp` when its declaration says it takes MCP servers
(`mcp_servers = "yes"`, a new capability) and purlis has no adapter for it, so no skill format
purlis can hand skills by. Any other harness has no route (`skills::route_of`).

The one line is the MCP server's `instructions`, ending with `skills::POINTER`, for a chat on
`Mcp` alone. A harness on that route runs none of purlis's hooks, so no `SessionStart` briefing
reaches it.

## Not built yet

- The compatibility badge's line for a load that cannot apply a skill's narrowing (SK-8, #2771).
- "Not supported by this harness" for a harness with neither a skill format nor MCP.
- `skill_find` over skills a router hid (SK-15, #2778).
