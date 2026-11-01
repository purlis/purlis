# A chat is told to load a skill its harness does not know

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-01**, ticket #4510, under the operator's spec #2762. ADR 0102 says nothing a router hides is out of reach. A skill whose text starts another by its harness's own tool could not rely on that. It also changes a line of [ADR 0063's 2026-10-20 record](../0063/2026-11-01-a-chat-on-a-handing-route-is-told-one-line-of-its-skills.md).

## The problem

A manual-only skill is never hidden, and the skills it names can be (ADR 0102 as amended 2026-10-25). `grill-with-docs` is one such: its whole body says to call the Skill tool for `grilling` and `domain-modeling`. A chat started before those were installed is in the same place, because its set's folder never changes. Its harness answers "Unknown skill", and `skill_load` would have answered from the whole set.

The index skill's description speaks of a request that may match a skill, not of a skill the harness refuses. Real Claude Code chats, siblings not handed, five runs per arm, `skill_load` served by a stub:

| Model | Arm | Loaded both |
|---|---|---|
| haiku | no index, no line | 1 of 5 |
| haiku | index, no line | 1 of 5 |
| haiku | no index, this line | 4 of 5 |
| haiku | index, this line | 5 of 5 |
| sonnet | no index, no line | 5 of 5 |
| sonnet | no index, this line | 5 of 5 |

These are small samples, so they show a direction. Wording matters: a first draft that did not say "before giving up" recovered 0 of 5 on haiku without an index. The stub offers two tools where the real server offers many. Only Claude Code was measured: whether Codex and opencode show a server's instructions to their model, and what a harness says of a skill it lacks, was not.

## The decision

- **The server's instructions say what to do.** A chat on a route that hands its harness skills (`Plugin`, `Config`, `Briefing`) is told `NOT_KNOWN`: when its harness says it does not know a skill, load that skill with `skill_load` before giving up or working around it. A chat on `Mcp` keeps `POINTER`, and a harness with no route is told nothing, as before.
- **Nothing reads a skill's prose.** The line depends on what the harness says to the model, and purlis does not decide anything from it.
- **`skill_load` takes a name in purlis's own namespaces only.** `by_name` read `other-plugin:deploy` as `deploy`, so following the line after a refused plugin skill could start an unrelated skill of the set. It now answers that the chat has no such skill.
- **It costs every such chat one line.** That line is not in the skill budget, since it is not a skill's description.

## Rejected

- **Only in the index skill's description.** A chat that started before the install has no index, and a chat under the budget has none either.
- **Keep a skill's named siblings listed.** It needs reading bare names out of prose, and would change what a router may hide.
- **Declared dependencies.** ADR 0099 adds nothing inside `SKILL.md`, and a source's skills do not declare any.

## What remains

- A skill that narrows the chat and was not handed still dead-ends: `skill_load` refuses it and sends the chat back to the harness's own start, which does not know it. That refusal keeps a narrowing from being skipped, so changing it is a decision of its own.
- The line does not stop on a manual-only skill: `skill_load` has never checked for one.
- The line is sent to a chat with nothing hidden. A chat started before an install needs it just the same, and nothing at the server's start can tell which it is.

The install review, `purlis skill uses` and the broken-reference lint read only `skills:<name>`, so a skill naming another by its bare name is still not reported to the person, and an install of it alone does not bring the other along. Telling the person needs a surface and a ruling on reading bare names.

**Built by #4510.**
