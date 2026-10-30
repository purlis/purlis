# A harness's own catalogue is shown beside the budget, never counted in it

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-31**, ticket #3324 (SK-39's follow-up), under the operator's spec #2762. It adds to [The skill budget and Use skill…, as built](2026-10-21-the-skill-budget-and-use-skill-as-built.md) and to [The budget leaves Use skill…'s row](2026-10-30-the-budget-leaves-use-skill-s-row-and-the-chip-warns-of-every-chat-need.md).

## The question

A chat's skill budget (`budget::spent`) counts the chat's set: the skills purlis hands it. Those are its built-in skills, the plane's, and the repo skills of the clone it works in, which a start has read since #3234. The skills a harness finds by itself, in the person's own folders and in its plugins, are never in that set. Codex keeps its own limit for that catalogue, about 2% of the context. Claude Code lists its own skills beside the ones purlis hands it.

The options were (a) leave the budget as it is and say so, (b) show the harness's own catalogue as a figure of its own that counts against nothing, or (c) count it.

## The decision: (b)

- **Each harness in the studio measures its own catalogue** (`skills::OwnCatalogue`, on `HarnessInTheStudio::own`). That is every skill it finds itself (`Origin::Harness`, the person's own and its plugins'), each turned off for the project's chats left out, and each purlis could not read left out. It is measured as purlis measures a chat's set on that harness (`budget::spent`, with its cap on a description and its paths where it lists them). A manual-only skill is apart, read as the harness reads it: by `disable-model-invocation` on a harness that reads that key (Claude Code), else by the file it keeps beside a `SKILL.md` (Codex's `agents/openai.yaml`, SK-39). A repo's skills are not in it, because purlis's budget counts those already.
- **It counts against nothing.** purlis never trims what it does not own. A harness's own figure does not move the budget's warning, and it does not add to the Needs you card.
- **It is said beside the budget**, in one set of words (`OwnCatalogue::said`): `Codex's own, not counted: 2 skills, 1,310 characters, 1 manual-only skill`. It is said in:
  - the Skills view's line under whose set it shows, after the budget, for the chat's own harness;
  - `purlis skill list`, under each harness, and as `own` in its JSON.
- **The studio's Routing page says what the budget counts**: the built-in skills, the project's, and the repo's. It also says that a program's own skills are not counted and are shown beside a chat's budget.
- **No read is added to a chat's open.** The figure is worked out where the studio already reads each harness's own skills (`harnesses_in_the_studio`), and only for a whole read (`Reading::Whole`). A read for a count of needs measures nothing (`own` is `None`). The Skills view takes it from the studio answer it reads anyway. `chat_skills`, which every open chat's chip reads, is unchanged.

## What remains

- **The harness's own limit is not shown.** Only its figure is shown. Codex's 2% depends on the model's context, and purlis does not know that.
- A skill purlis could not read (a link in a clone, a file too large) is left out of the figure, though the harness may still list it.
