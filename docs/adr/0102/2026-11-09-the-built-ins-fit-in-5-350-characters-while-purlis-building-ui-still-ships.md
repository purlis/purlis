# The built-ins fit in 5,350 characters while purlis-building-ui still ships

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-09**, spec #5683 (PR #5703). It suspends two numbers of the [amendment of 2026-11-08](2026-11-08-the-built-ins-fit-in-5-000-characters-and-opencode-lists-8-000.md) for a time, and changes nothing else of it.

**Who decided what.** The chat that carries spec #5683 decided it on 2026-11-09, option A of three, as a temporary measure, when purlis-report could not join the built-ins under the 5,000 cap. The person rules on it by merging the pull request that carries this record.

## Why

Spec #5683 adds an 18th built-in, `purlis-report`, held to the strict bar. The other 17 built-ins' descriptions already came to 4,997 of the 5,000 characters, so no description of purlis-report fits. One of them, `purlis-building-ui`, is leaving the bundle for the plane (#5725, already on the shipped-skill check's `LEAVING` list), and its description is 240 characters. Until it leaves, the 5,000 cap holds 17 built-ins and no 18th.

The alternatives were to trim each other built-in by about 18 characters, which would change descriptions whose trigger bars are measured and not yet re-measured, or to take `purlis-building-ui` out of the bundle now, ahead of #5725's own work.

## What holds now

| Number | Was | While `purlis-building-ui` ships | After #5725 |
|---|---|---|---|
| Every built-in's description together, at most | 5,000 | 5,350 | 5,000 |
| What the built-ins leave of Codex's 8,000 for the plane's skills, at least | 2,800 | 2,500 | 2,800 |

- **Both numbers are written once**, as `purlis_core::skills::BUILT_IN_DESCRIPTIONS_MOST` and `purlis_core::skills::CODEX_LEFT_FOR_THE_PLANE`, each with a comment naming #5725. The shipped-skill check (`crates/purlis-cli/tests/it/every_shipped_skill_passes_the_built_in_checks.rs`) and `the_built_ins_codex_listing_leaves_room_for_the_planes_skills` read them.
- **The change that takes `purlis-building-ui` out of the bundle puts both back** to 5,000 and 2,800, in the same change.
- **purlis-report's description is about 300 characters**, near the house style of about 290 (`docs/development.md`).
- Codex's and opencode's default budgets stay 8,000. The 1,024-character cap on one description stays.

## What it costs

Until #5725 lands, a Codex chat has about 300 fewer characters for the plane's own skills than the 2026-11-08 amendment promised: at least 2,500 rather than 2,800.
