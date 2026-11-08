# Claude Code's limit holds the person's own skills, and Codex lists built-ins first

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, ticket #5736, under the operator's spec #5714 (decisions Q1–Q36, settled 2026-11-08). It changes what [A chat's skill budget is its harness's](2026-11-02-a-chat-s-skill-budget-is-its-harness-s-and-the-setting-overrides-it.md) hands Claude Code, and adds to [A harness's own catalogue is shown beside the budget, never counted in it](2026-10-31-a-harness-s-own-catalogue-is-shown-beside-the-budget-never-counted.md).

## The question

The 2026-11-02 record hands Claude Code an absolute limit on its skill listing, and says a person's own skills share it uncounted. Claude Code counts them, so a person with many skills and plugins still has descriptions dropped, and the model then sees bare names for some of the skills purlis hands. purlis already measures that catalogue for the Skill studio.

## The decision

- **The limit Claude Code is handed holds the person's own catalogue.** A chat's start reads the skills its harness finds itself (the person's own and its plugins'), with the plugins it holds (`hoststart::ChatSkills::own`), and measures them as the studio does (`skills::own_shares`). The limit is purlis's list, that figure and Claude Code's own 6,848, and a tenth over all of it, never below 40,000.
- **The person's share never lifts it past 100,000, Claude Code's own ceiling.** 100,000 is the most a listing limit Claude Code sets itself may reach (2.1.319's code). purlis's own list is never cut for the person's: where it needs more than the ceiling by itself, the limit holds it, as a raised budget always raised the limit.
- **Past that, the chat says who takes the room.** Where purlis's list, the person's catalogue and Claude Code's own entries together pass the limit handed, the chat's Skills view has a need, *Some descriptions are dropped*, naming the three biggest contributors with their characters: each plugin by name, the person's own skills, and purlis's skills. Its fix opens Harness skills, where they are turned off for the workspace's chats. It counts in the pane's warning as every chat need does.
- **purlis's budget still never counts them.** The router hides nothing for them (the 2026-10-31 records), and the budget line still says the program's own catalogue as "not counted": it is in the limit Claude Code is handed, not in purlis's budget.
- **Codex's briefing lists purlis's built-in skills first.** Codex drops trailing skills past its limit without saying so. A briefed chat's list names the built-in skills before the plane's and the index skill, whatever their names, so they are counted first against Codex's limit and are never the ones past it.

## What remains

- A chat the plane's skills do not reach (the Skill studio's flag off) is handed its limit without the person's catalogue, as before.
- The Skills view measures the catalogue the chat held at its start where its start held one, else as it is now.
- Codex's limit is still not measured again (the 2026-11-02 record), so purlis does not say when Codex would drop its own skills.

**Built by #5736.**
