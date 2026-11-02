# A chat's skill budget is its harness's, a setting overrides it, and Claude Code is handed its limit

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-02**, ticket #4610. It replaces "one number for every harness" in the [amendment of 2026-10-21](2026-10-21-the-skill-budget-and-use-skill-as-built.md), and corrects that record's Claude Code row.

**Who decided what.** The operator ruled: a larger default; each harness's real limit rather than one number for all, with a margin; `settings.skills.budget` at project and workspace level with a Settings row; a value above a harness's limit warns and says what the harness would drop, and is never refused; past the budget the router still hides all hideable skills, and pins stay the way to keep specific ones. **The steward decided the means below while the operator slept, and the operator is to review it:** handing Claude Code an absolute limit, and the numbers that follow from it.

## What the measurement found

Measured on Claude Code 2.1.316 (session record `20261102-022144`):

| Window and model | Claude Code's limit |
|---|---|
| 1M, Claude 5 | 30,000 |
| 200k, Claude 5 | 6,000 |
| 200k, Haiku 4.5 | 8,000 |
| 1M, Opus or Sonnet 4.6 | 40,000 (from its code, not run) |

The 2026-10-21 record's "about 8,000 characters on a 200k-token window" holds for Haiku 4.5 alone, and its "so the listing stays under its own cut" does not hold: the limit counts Claude Code's own 14 entries, which take 6,848 characters. **On a 200k Claude 5 chat every plane skill's description was dropped, however few skills it had.** Past a limit Claude Code keeps every name and drops descriptions, and says so only in a `[WARN]` in its debug log, so the model just sees bare names.

## A chat's window isn't known at its start, so the start is not re-ordered

The router gets the budget before the harness runs, and the window depends on model, provider, account and a credit latch. A budget by window would need a value that isn't there when it is needed. So the limit stops depending on the window.

**The Claude Code adapter hands an absolute limit per chat**: `SLASH_COMMAND_TOOL_CHAR_BUDGET` in the `env` of the `--settings` it already passes. Measured, it ignores the window and beats `skillListingBudgetFraction`. The limit is the budget's characters plus Claude Code's own 6,848, and a tenth over both, and never below 30,000, a 1M Claude 5 chat's own limit, so a larger default takes no room from anyone. It is a ceiling: a chat pays only for what it lists, and a person's own skills share it. So at the default it is 30,000, where the sum alone would be about 24,000. Where pins and skills no router hides spend more than the budget, the limit is worked out from what the routed list spends instead, so they keep their descriptions.

## Each adapter says its own numbers

| Harness | Budget | What counts | Its limit |
|---|---|---|---|
| Claude Code | 60 skills, 15,000 characters | Whole lines: name, description and `when_to_use` up to 1,536, and 23 characters around them | Handed, so it rises with the budget |
| Codex | 30 skills, 6,000 characters | As before: description up to 1,024, and the path | Its own: 2% of the window, 8,000 where it doesn't know the window. Not measured again |
| opencode | 30 skills, 6,000 characters | As before | None |

`skill_budget` stays argument-free. `crate::skills::budget_in` lays the setting over it, and every place that decides by a budget asks that.

Codex and opencode stay where they are until Codex is measured again. The Skill studio's "on every program" goes: it says each harness's budget.

## `settings.skills.budget`

`{skills = 80, chars = 20000}`, either key or both, in `[skills.budget]` of the project's committed file and in `settings.skills.budget` of a `workspace.json`, whose keys win one by one. It overrides the adapter's numbers on every harness.

- **A raised Claude Code budget raises the limit it is handed.**
- **Past Codex's limit, Settings says so and what Codex would drop.** Nothing is refused for being large. Only what no reader takes is refused: a value that isn't a whole number of at least 1.
- **Claude Code never warns**, because its limit follows the budget. opencode has none.
- **A router extension is told each skill's cost.** `route` hands each skill a `chars`, what it costs of the budget on that harness, since on Claude Code that is its whole line. A router adds those up.
- **A running chat keeps what it was handed.** Use skill… works a chat's set out now, as the 2026-10-21 record says, so a budget changed after a chat started shows there at once, and its restart offer is the answer. The limit it was handed is the one from its start.

## What this costs, and what isn't measured

- **About 91 tokens per skill**, measured on Haiku 5.5 with 250-character stand-in descriptions, cached: the listing is written once and read as cached prefix after that. Real prose may differ. 60 skills is about 5,460 tokens, 0.55% of a 1M window.
- **A person's own skills share the handed limit and are not in it.** Claude Code counts them, and purlis doesn't read their descriptions at a start. A 1M Claude 5 chat keeps its 30,000. A 200k one with a large set can now spend up to about 5% of its window (30,000 characters, about 10k tokens). A 1M Opus or Sonnet 4.6 chat falls from the 40,000 its code gives to 30,000, not run.
- **That the variable beats the person's own environment, and how managed settings rank, are not measured.** The docs say a settings `env` entry replaces the shell's in most sessions, that a launch environment from Claude Desktop or a self-hosted runner goes first, and that managed settings outrank `--settings`.
- **Compaction is not measured.** The docs say skill descriptions are re-injected after one, from 2.1.304.
- **Support for the variable** was first read from the binary. `env-vars.md` documents it, as a legacy name.
- **Codex's 2% and 8,000 are the 2026-10-21 record's.** Its binary now holds "omitted from this bounded skills list", not run.
- **The line's 23 characters** are about 16 measured and 7 for `skills:` or `purlis:`.
