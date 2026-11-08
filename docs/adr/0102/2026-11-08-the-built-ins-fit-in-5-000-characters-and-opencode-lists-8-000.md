# The built-ins fit in 5,000 characters, and opencode lists 8,000

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, spec #5714. It changes opencode's row of the [amendment of 2026-11-02](2026-11-02-a-chat-s-skill-budget-is-its-harness-s-and-the-setting-overrides-it.md).

**Who decided what.** The person ruled (spec #5714's Q37, 2026-11-08, after CI showed it): every built-in skill's description together is at most 5,000 characters, enforced by the shipped-skill check, and opencode's default budget rises to 8,000. The 1,024-character cap on one description stays.

## Why

The 17 built-ins' descriptions came to 10,823 characters, past opencode's and Codex's 6,000 on their own, so the router hid every plane skill on both. Codex lists skills in 8,000 characters where it does not know the window and drops what does not fit, so 5,000 for the built-ins leaves about 3,000 for the plane's own skills there.

## What changes

| Harness | Budget | Change |
|---|---|---|
| Claude Code | 60 skills, 15,000 characters | None |
| Codex | 30 skills, 6,000 characters | None, until it is measured again |
| opencode | 30 skills, 8,000 characters | Was 6,000 |

The shipped-skill check (`crates/purlis-cli/src/main/tests/shipped_skills.rs`) fails when the built-ins' descriptions together pass 5,000 characters, and names the biggest. `docs/development.md`'s house style says about 290 characters each.

## What it leaves

- **Codex counts each skill's path too**, and its purlis budget stays 6,000. With the bundle beside the binary, the built-ins' descriptions and paths still pass it, so on Codex the router still hides plane skills until a budget is raised. Tests that list plane skills on Codex keep their own `[skills.budget]`.
- **The trigger bars are not measured on the shorter descriptions.** The CI grader measures them once its keys exist.
