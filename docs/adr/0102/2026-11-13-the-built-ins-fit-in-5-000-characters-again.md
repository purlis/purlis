# The built-ins fit in 5,000 characters again

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-13**, ticket #6445. It ends the two temporary numbers of [the built-ins fit in 5,100 characters once purlis-building-ui is retired](2026-11-12-the-built-ins-fit-in-5-100-characters-once-purlis-building-ui-is-retired.md), as that record said the change trimming the descriptions would.

## Why

Six built-ins' descriptions are trimmed by cutting words that said a thing twice. Every `Use when` sentence and every `Not for` neighbour stays, and so does the word a neighbour points by (`purlis-secrets`' "credential"):

| Skill | Before | After | What went |
|---|---|---|---|
| `purlis-secrets` | 324 | 318 | "entering the chat" is "in the chat" |
| `purlis-report` | 319 | 312 | the second "purlis" of "When purlis itself misbehaves — a purlis command, …" |
| `purlis-handoff` | 322 | 291 | "they will read" is "they read"; "work this chat needs an answer from, its own persona's or another's" is "work this chat needs a persona's answer to" |
| `purlis-workspace` | 304 | 301 | "chosen or shared, or saved" is "chosen, shared or saved" |
| `purlis-remember` | 298 | 285 | the first of its two "add a todo"s |
| `purlis-safe-remove` | 300 | 280 | "remove it without losing what it learned" is "keep what it learned" |

The 17 built-ins' descriptions together come to **4,983** characters, down from 5,063. Codex's 8,000-character listing now leaves **2,877** for the plane's own skills, up from 2,797, measured by `the_built_ins_codex_listing_leaves_room_for_the_planes_skills`. (The 5,081 the earlier record gave is the same descriptions counted in bytes. The checks count characters.) The `evals/sources.json` lines that quoted `purlis-safe-remove`'s old words quote its new ones.

## What holds now

| Number | Until now | Now |
|---|---|---|
| Every built-in's description together, at most | 5,100 | 5,000 |
| What the built-ins leave of Codex's 8,000 for the plane's skills, at least | 2,750 | 2,800 |

- These are spec #5714's own numbers (Q37 and Q38), still written once, as `purlis_core::skills::BUILT_IN_DESCRIPTIONS_MOST` and `purlis_core::skills::CODEX_LEFT_FOR_THE_PLANE`.
- A built-in whose description grows takes the characters from another built-in's (`docs/development.md`, the house style). Only 17 characters are spare under 5,000.

## What it costs

Each trimmed description says its situation in fewer words. Its trigger tests are unchanged and are graded against the same bar.
