# A manual-only skill is loaded through `skill_load` only when the person asked for it

**Amends [ADR 0063](../0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md), 2026-11-01**, ticket #4529, under the amendment of 2026-10-20 that gave a chat `skill_load`. `disable-model-invocation: true` keeps a skill out of its model's choice on every harness, but `skill_load` did not read the key, so a chat could start one by itself.

`skill_load` now treats it as a harness does. Without `person_asked: true`, the skill is not handed to the chat and no run opens. The answer is the same on every harness: "`<name>` is manual-only, so it was not loaded. Load it again with person_asked true only if the person named this skill. Otherwise ask the person to run it."

With `person_asked: true`, it loads and opens its run. This is the operator's ruling of that day, relayed by the chat that asked for this work. Without it, a chat with no start of its own (the `Mcp` route, or a terminal's own Claude Code, which was handed none of the plane's skills) left a person no way to run such a skill.

**This stops a skill firing by itself. It is not a security boundary.** The argument is the model's own word, and nothing checks it.

Use skill… passes it wherever it asks a chat to load a skill. A manual-only skill reaches that path only if a router comes to hide one: today it types the harness's own start.

## Not fixed

`skill_list` and `skill_find` still list a manual-only skill with its description.
