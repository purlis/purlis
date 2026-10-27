# Amended 2026-10-28: a grade's chat runs with only purlis's plugin

Ticket #3468, the operator's ruling P8 of 2026-10-27 on spec #2762 (round 2 of the harness
plugins grilling). It adds to §3, which program a grade's chats run on and how they are held.

**A grade measures the skill alone, so its chats have every plugin off but purlis's own.** A
harness plugin brings skills, hooks, MCP servers and subagents of its own, and any of them can
change whether the skill under test triggers. So a grade's chat is handed every plugin its
harness has installed off, by that harness's own per-chat switch (Claude Code's `enabledPlugins`
in the chat's `--settings`; Codex's `-c plugins."<id>".enabled=false`, once Codex can grade),
with purlis's pins written over them, so purlis's own plugin is the one on
(`harness_plugin::only_purlis_on`, asked by `skills::grades::ready_to_grade`). Nothing of the
person's own settings is written: a chat outside the grade has its plugins as the harness's
settings say. A plugin record purlis cannot read refuses the grade, since a grade that could not
say which plugins were off would be read as measuring the skill alone.

**The record says which were off.** Each row of the eval record names them, by the harness's id,
in `plugins_off` (purlis's pins left out). A row written before this has none.

**opencode never grades.** Its plugins are code it loads into its own process, and it has no
switch that turns one off for a single chat. `can_grade` now answers with the reason where a
harness cannot grade, so the refusal names it: opencode's is this, in one sentence, and Codex's
is still its limits and spend (#3272).

## Rejected

- **Turning the plugins off in the person's own settings for the grade's run.** It would write
  the person's files and leave them changed if the grade were stopped by a crash.
- **Grading opencode with its plugins on and saying so.** The ruling is that a grade measures the
  skill alone; a grade with the person's plugins in it would not be comparable with one without.
