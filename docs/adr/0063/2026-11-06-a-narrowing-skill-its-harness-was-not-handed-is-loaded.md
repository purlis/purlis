# A narrowing skill its harness was not handed is loaded, saying its narrowing is not applied

**Amends [ADR 0063](../0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md), 2026-11-06**, ticket #4510, under §2 of [a load that would lose its narrowing is refused](2026-10-21-a-load-that-would-lose-its-narrowing-is-refused-and-sent-to-the-harness-s-own-start.md). It closes the first item that [ADR 0102's record of 2026-11-01](../0102/2026-11-01-a-chat-is-told-to-load-a-skill-its-harness-does-not-know.md) left open.

## The problem

On Claude Code, `skill_load` refused every skill that narrows the chat (`disallowed-tools` or `context: fork`) and told the chat to start it as `skills:<name>`. That refusal is right only for a skill the harness was handed. A plane skill the router hid, or one made after the chat started, is not in the set's folder its harness loaded, and that folder never changes. The harness answers "Unknown skill". The server's `NOT_KNOWN` line then sends the chat back to `skill_load`, which refuses again. Nothing could start the skill: `grill-with-docs` starting a hidden `to-tickets` that forks hit this every time.

## The decision

**The refusal holds only for a skill the chat's harness holds.** A Claude Code chat is started with `PURLIS_SKILLS_HANDED`, set by its adapter, which names the set's folder of its second `--plugin-dir`. purlis's MCP server is started by the harness with that environment. `skill_load` sends a chat to `skills:<name>` only where that folder holds the skill's `SKILL.md` (`skills::handed_in`). A built-in skill is always in the bundled plugin, and a harness's or repo's own skill is loaded by the harness, so the check covers only the plane's skills.

**A plane skill its harness does not hold is loaded**, opens its run, and the answer says "Narrowing not applied". The same rule already holds where no harness start can apply the narrowing: a terminal's own Claude Code, Codex, opencode and the `Mcp` route. A refusal would do nothing but stop the skill from running. A chat named no folder is treated as holding none of the plane's skills. That covers a chat started before this change.

## Rejected

- **Never hide a narrowing skill.** That would change what a router may hide, and it does nothing for a skill made after the chat started.
- **Recompute what was handed in the server.** The server cannot route the set again the way the chat's start did: a router extension's answer, and the skills as they were at the start, are not in reach. The folder is what the harness actually holds.
- **Read the harness's own command line for `--plugin-dir`.** That would read the harness's process, not something purlis handed it.

## What remains

A skill loaded this way runs with its narrowing dropped, as it already does on every harness that cannot apply one. A new chat is handed the skill, if its router keeps it, and starts it with its narrowing.
