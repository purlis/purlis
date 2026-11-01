# A manual-only skill is not loaded through `skill_load`

**Amends [ADR 0063](../0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md), 2026-11-01**, ticket #4529, under the amendment of 2026-10-20 that gave a chat `skill_load`. `disable-model-invocation: true` keeps a skill out of its model's choice on every harness, but `skill_load` did not read the key, so a chat could start one by itself.

`skill_load` now treats it as a harness does. The skill is not handed to the chat and no run opens. The answer is the same on every harness: "`<name>` is manual-only, so it was not loaded. Ask the person to run it."

Use skill… does not rely on this path for such a skill: no router hides a manual-only skill, so it types the harness's own start.

## Not fixed

A chat with no start of its own, on the `Mcp` route or a terminal's own Claude Code, which was handed none of the plane's skills, leaves a person no way yet to run a manual-only skill. The answer still tells the chat to ask them, as the tool cannot tell a model's own pick from a person's request.

`skill_list` and `skill_find` still list a manual-only skill with its description.
