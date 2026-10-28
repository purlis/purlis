# A router is graded on its library's trigger tests, as #3233 built it

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-28**, ticket #3233 (SK-15e). ADR 0102 §8 says that a router is graded with the library's trigger tests, against the built-in router, and that the grade is recorded with the library's commit. This records which library that is, how its chats are started, and where the grade is kept and shown. The chats are a grade's chats as [ADR 0105](../0105-a-grade-is-real-chats-purlis-runs-cost-first-scored-by-skill-runs-and-recorded-against-the-skills-version.md) built them, with the same sandbox, the same stripped-down tools, the same token cap and the same dollar limit.

## Which router, and which library

The router graded is the one the Skill studio's workspace or persona chose (§5). Only an extension's router is graded: the built-in router is the baseline.

The library is the set a chat of that workspace, as that persona, is handed. That is every project, workspace and persona skill of it, at most 200, each read once for the grade. Built-in skills are in every chat anyway. Every chat of the grade is handed the whole library, so the router is asked about the same candidates a real chat's start asks it about. Only the skills with trigger tests are graded.

## How each chat starts

Each chat starts in a throwaway piece of its own, laid out from that one read: every skill without its `evals/` folder, marked as a grade's arm. The arms are:

- **`router`**: the piece also names the router. The chat's start asks it, as any chat's start does (SK-15b), instead of the workspace's or persona's choice. A chat whose start fell back on the built-in router (slow, failed or wrong) measured that router instead. So it is no run: it is ended, and the grade says why. Two in a row end the grade, as chats that do not start do.
- **`built-in-router`**: the baseline. It is routed by the built-in router, and nothing is asked.

A test passes in an arm by the share of its runs that started the skill, as a skill's trigger test does. A run of the skill counts whichever route started it: the harness's own pick, or a load from the index through `skill_load`. Both open a skill run, which is all a trigger test is scored by.

## The cost, the consent and the limits

The plan says what grading runs before anything does:

- the chats, the tests and the skills;
- the library each chat is handed;
- the cap and the dollar limit;
- the profile and the sandbox;
- whose account pays;
- the commit it would be recorded on.

Pressing Grade hands back one digest of the router, the scope, every test and every copy of the library. A grade started on a plan that no longer holds is refused.

The skills are graded one after another, held to one cap and one dollar limit over them all. A grade that stops, at its cap, its limit or the person's Stop, keeps what its skills scored so far. One grade runs in a project at a time, a skill's or a router's.

## The record, and where it is shown

A router's grade goes to `personas/_grades/routers/`, beside the skills' eval record, which never reads it. One row is one grade, and it holds:

- the router and the scope that chose it;
- the library's commit: the plane's `HEAD` where every copy was committed there, else none;
- every copy's version;
- each graded skill's report, with the arms `router` and `built-in-router`.

A library with a copy that is this machine's alone is graded into this machine's state instead.

The studio's router line, where an extension's router is chosen, says the router's newest grade: how many trigger tests pass with it and with the built-in router, and the library's commit it was graded on. It also offers Grade the router….

## What this does not do yet

Each of these follows as its own ticket in M65:

- A router's grade under way is not held for a crash: one purlis stopped under is not recorded as interrupted, as a skill's is (#3353).
- What its chats showed is not kept for a scorecard (SK-59).
- The plan runs on the project's default profile and its model alone, with no pickers (SK-60).
- A router that reads prompts (SK-15c) is graded on its start's answer alone.

**Built by #3233.**
