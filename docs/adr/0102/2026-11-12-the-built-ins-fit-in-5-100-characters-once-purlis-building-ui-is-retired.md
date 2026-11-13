# The built-ins fit in 5,100 characters once purlis-building-ui is retired

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-12**, ticket #5725. It replaces the two temporary numbers of [the built-ins fit in 5,350 characters while purlis-building-ui still ships](2026-11-09-the-built-ins-fit-in-5-350-characters-while-purlis-building-ui-still-ships.md). That record said the change taking `purlis-building-ui` out of the bundle would put both numbers back to 5,000 and 2,800. That change cannot keep its promise.

## Why

With `purlis-building-ui` retired ([ADR 0099 as amended 2026-11-12](../0099/2026-11-12-purlis-building-ui-is-retired-with-no-skill-in-its-place.md)), the 17 built-ins' descriptions come to **5,081** characters. Codex's 8,000-character listing then leaves **2,797** for the plane's own skills, measured by `the_built_ins_codex_listing_leaves_room_for_the_planes_skills`. `purlis-report` (323 characters) took more room than the retired skill (239) freed, so neither number of spec #5714 (Q37's 5,000, Q38's 2,800) holds yet.

Getting there means trimming about 81 characters from the built-ins' descriptions, whose trigger grades are measured against their bars. That is a change of its own, not this one.

## What holds now

| Number | While `purlis-building-ui` shipped | Now | Once the descriptions are trimmed |
|---|---|---|---|
| Every built-in's description together, at most | 5,350 | 5,100 | 5,000 |
| What the built-ins leave of Codex's 8,000 for the plane's skills, at least | 2,500 | 2,750 | 2,800 |

- Both numbers stay written once, as `purlis_core::skills::BUILT_IN_DESCRIPTIONS_MOST` and `purlis_core::skills::CODEX_LEFT_FOR_THE_PLANE`. Each is tightened to the nearest round number that holds now.
- **The change that trims the descriptions puts both back** to 5,000 and 2,800, in the same change. The longest descriptions are `purlis-secrets` (328), `purlis-report` (323) and `purlis-handoff` (322), against a house style of about 290 (`docs/development.md`).

## What it costs

Until then, a Codex chat is promised at least 2,750 characters for the plane's own skills, not 2,800. It gets 2,797.
