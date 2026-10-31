# Amendment, 2026-10-27: purlis stops choosing other harnesses' plugins, and keeps its own

The operator's rulings of 2026-10-26 and 2026-10-27 on spec #2762 (P1, P2, P3, P6, P7, P9 and
P10, all "agree"), built by #3467. **This supersedes the per-project choice the record above
made**, and the Codex and opencode parts of the 2026-10-21 amendment that were about plugins.

## Why

A plugin is more than its skills. It can also carry hooks, MCP servers, subagents, LSP servers
and code that runs inside the harness, and Claude Code, Codex and opencode each handle every one
of those differently. Only skills and MCP are portable across the three (research of 2026-10,
agent-plugins.org). A per-project on/off switch for another harness's plugin therefore promised
something purlis could keep on one harness, half keep on another (Codex took only "off"), and
not keep on the third (opencode has no per-chat switch). The harness already has its own answer
for its plugins. purlis now leaves it there.

## The decision

- **purlis does not choose another harness's plugins.** The per-project switches are removed:
  `[harness_plugins]` in `charter.toml` and `charter.local.toml`, a workspace's
  `settings.harness_plugins`, the Settings tab's Plugins group, the Skill studio's plugin rows
  and their Turn off / Leave to Codex buttons, and `project_harness_plugins`. Which plugins load
  is the harness's own settings' to say.
- **purlis keeps its own plugin** (**purlis's plugin**, CONTEXT.md): `--plugin-dir` for Claude
  Code, the `-c hooks.*` flags for Codex, the opencode shim (ADR 0058), and `purlis plugin
  install` for the chats started outside the app (ADR 0057).
- **The only plugin ids purlis writes are its pins.** A Claude Code chat's `--settings`
  `enabledPlugins` holds `purlis@inline` on and the Python charter's `charter@charter` off, as
  before, and the ids purlis's own plugin had before the rename (`charter@inline`,
  `charter-app@inline`, `charter@charter-app`) off, which are the same plugin under its old
  names, so an old copy never loads beside the new one. Codex and opencode pin nothing.
- **A Try it's scratch chat still turns every installed plugin off** for that one chat (decision
  50): that is purlis's own chat, not a project's choice, and it is why `Adapter::installed`
  stays for Claude Code and Codex. opencode lists none, since it cannot turn one off.
- **A plugin's skills are shown read-only.** The Skill studio and `purlis skill list` show each
  skill a plugin brings, with the plugin named as its source, and no switch: on every harness a
  `[harness_skills.<harness>]."<plugin>:<skill>"` is refused by the save and handed to no chat
  (`Adapter::skill_named_alone`). A skill in the person's own folders, or a repo's, can still be
  turned off for a project, as the 2026-10-21 amendment built.

## Old keys

A project that still holds `[harness_plugins]` starts as any other. The keys are **ignored and
never rewritten behind the person's back**: no reader reads them, no save refuses them, and a
save of another setting keeps them as they are. The window says so **once**, in one ask in the Inbox (purlis's own, of no chat) that
names each file holding them and that each harness decides. Its **Remove** holds a proposed
change for each of the project's two files (the change purlis itself proposes, judged by
`proposed::proposable_by_purlis` and drawn and applied as a chat's proposal is, #1975): nothing is
written until the person presses Apply. **Dismiss** keeps the ask away. Either answer is kept
in the project's own state folder on this machine. A workspace's `settings.harness_plugins` gets
a proposed change of its own too (#3486), worked out as a form's save would write the manifest
without the key and, on Apply, written by the manifest's own writer
(`settings::workspace::save_text`) under its lock, pinned to the manifest as it was read. So a
manifest purlis wrote keeps its stamp and one a hand wrote stays the hand's, as after a form save.

## What this amends

- **Spec #2762, decision 25** ("plugin on/off on all three harnesses"): amended. Harness skills
  turn on and off per skill where the harness allows, and never per plugin.
- **Spec #2762, decision 35** ("every skill choice lives in the studio, including harness plugin
  on/off"): amended. There is no plugin choice left to put in the studio, so the Settings tab's
  Plugins summary is gone too.
- **ADR 0048**: its workspace layer and its "a Local file git would carry decides nothing" now
  reach `[harness_skills]` alone among a harness's tables.

## What was rejected

- **Keeping the switches for Claude Code only.** It would leave the one harness where purlis
  could switch a plugin behaving unlike the other two, which is what the operator's
  harness-agnostic rule (ADR 0050) asked purlis not to do.
- **Rewriting old keys away on a save.** A file is the person's: the ruling is that purlis says
  so and offers an edit they approve, and never changes the file by itself.
- **Turning a plugin's skills off one by one where a harness allows it (Codex).** A plugin's
  skill comes with the plugin, whose other pieces would stay on; showing it read-only says that
  plainly.
- **Managing a plugin's other pieces.** MCP servers, subagents and hooks are idea tickets of
  their own (#3423, #3424, #3425), and Agent Plugins is watched in #3426.
