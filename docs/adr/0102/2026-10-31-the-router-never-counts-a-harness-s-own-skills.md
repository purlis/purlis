# The router never counts a harness's own skills

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-31**, ticket #4192, under the operator's spec #2762. It builds what [A harness's own catalogue is shown beside the budget, never counted in it](2026-10-31-a-harness-s-own-catalogue-is-shown-beside-the-budget-never-counted.md) left open: the studio's harness page routed its preview with the harness's own skills counted.

## The decision

- **The router counts nothing a harness finds itself.** `SkillSet::routed` and an extension router's candidates leave every `Origin::Harness` skill out of what the listing spends. A chat's start never holds such a skill, so its routing is unchanged. A set that does hold them now routes exactly as that start does.
- **A chat's start and the studio route through one function**, `skills::as_a_start_routes`: the skills from outside the plane beside the set, then the budget, by the start's chosen router or else the built-in one. At a start, the skills from outside the plane are the repo skills of the chat's own clone. In the studio they are also the harness's own skills, which stay in the set for its clashes and rows.
- Clashes with the harness's own skills are unchanged: they are added beside the set in the same order as before.

## What remains

- The studio routes with the repo skills of every clone in the workspace, and by the built-in router. A chat's start uses only its own clone's repo skills, and the router its persona or workspace chose: #4215.

**Built by #4192.**
