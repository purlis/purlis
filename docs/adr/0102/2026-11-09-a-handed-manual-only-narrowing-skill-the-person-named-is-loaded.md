# A handed manual-only narrowing skill the person named is loaded, saying its narrowing is not applied

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-09**, ticket #5880. #4510 left this case open (PR #5875). It follows the rule of [ADR 0063's record of 2026-11-06](../0063/2026-11-06-a-narrowing-skill-its-harness-was-not-handed-is-loaded.md).

## The problem

A Claude Code chat is handed a plane skill that is manual-only (`disable-model-invocation: true`) and also narrows the chat (`disallowed-tools` or `context: fork`). The person names it in prose, so the chat calls `skill_load` with `person_asked`. Because the skill narrows and the harness holds it, `skill_load` refused it and answered *"Start it with this harness's own skill tool as `skills:<name>`"*. Claude Code reads `disable-model-invocation` itself, so its Skill tool refuses a model-started `skills:<name>`. The chat stopped there, with nothing left to try.

That answer is the only text that sends the chat to `skills:<name>`. Before calling `skill_load`, the chat could not try the Skill tool by itself: a manual-only skill is left out of Claude Code's listing. The briefing, the index skill and the MCP server's instructions (`NOT_KNOWN`) all point to `skill_load`. Reproduced in `skills/routing/tests.rs` (`a_handed_manual_only_narrowing_skill_…_5880`).

## The decision

**`skill_load` sends a chat to its harness's own start only where that start would run the skill for the model.** A harness that reads the manual-only key itself (`ManualOnly::Read`, Claude Code today) never starts a manual-only skill for its model. So a manual-only skill there has no narrowed start (`skilltools::Asking::narrowed_start`). When the person asked for it, the skill is loaded, opens its run (started by a load through MCP), and the answer says "Narrowing not applied", as for an unhanded skill. When they did not ask, it is still not loaded, as #4529 has it.

A harness that keeps a manual-only skill out some other way (`Listing`, `WithoutDescription`) still runs one by name. None of them has a narrowed start today, so nothing changes there. If one gains a narrowed start, its refusal holds, since its own tool would start the skill.

## Rejected

- **Ask the person to type `/skills:<name>`.** That would keep the narrowing, but the person already named the skill, and the chat would end its turn on a request to name it again. The person can still type it to get the narrowing, and the changelog says so. The model's answer does not suggest it, so a turn never ends on a second request.
- **Hand Claude Code a manual-only narrowing skill without `disable-model-invocation`.** Its own tool could then start the skill, but so could the model, unasked. That breaks what manual-only means.
- **Decide in the adapter.** `narrowed_start` names how a harness starts a skill. Whether the harness refuses a manual-only one already comes from `skill_frontmatter`'s `manual`, so the check reads that instead of adding a second answer.

## What remains

A skill loaded this way runs without its narrowing, as it already does on every harness that cannot apply one. `person_asked` is the model's word (#4529), so this is no security boundary.
