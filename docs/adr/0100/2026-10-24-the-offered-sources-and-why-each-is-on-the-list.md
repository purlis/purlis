# The offered sources, and why each is on the list

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-10-24**, ticket #2785 (SK-22; spec #2762 decision 42). §1 says the studio's first open "offers the first-party catalogue and well-known sources with one click". This file records what that list is, who decides what goes on it, what one click does, and what the catalogue is today.

## The list is committed data, read like everything else from outside

- The list is `crates/purlis-core/src/skills/first_open/offered.toml`, built into purlis. Each table names its `offering` (`catalogue` or `well-known`), a name, an `https://` URL, an optional branch, a folder and one line that says why it is offered. The window shows that line beside the name.
- It is read into a closed shape (`deny_unknown_fields`). Each source must pass the check that a team's committed `[[skill_sources]]` passes: an `https://` URL with a plain host, a plain folder, no sign-in. An entry that fails takes the whole list away, and a unit test fails the build before that can ship.
- purlis still resolves no name and runs no index (§1, §7). The list is the whole of what purlis offers, and it changes only through a reviewed pull request, like any code.

## What one click does

The click opens Add source filled in with the source and asks: *Add it to this project's skill sources? Nothing is installed until you review a skill from it.* The person can change where it is kept (the project or this machine) before Add it. An offered source never skips SK-10's steps. Adding it lists it, Browse fetches it, and each install is its own review of the exact files, summary first (§3). A source the project already browses, whatever the spelling of its URL (a trailing `.git` or `/`, letter case), is not offered again. A project whose sources cannot be read is offered none, since an add would be refused.

## Which sources are on it, and why

A source is on the list when all of these hold: it is a public git repository of `SKILL.md` folders; it is kept by the maker of a chat program purlis runs, or it is a collection people use widely across those programs and it is still maintained; and its skills are general, not tied to one company or product.

| Source | Folder | Why |
|---|---|---|
| `https://github.com/anthropics/skills` | `skills` | Anthropic's collection, where the open skill format comes from, kept by the makers of Claude Code. |
| `https://github.com/openai/skills` | `skills/.curated` | The skills OpenAI curates for Codex, kept by its makers. The `.system` folder beside it holds Codex's own built-in skills, so it is left out. |
| `https://github.com/obra/superpowers` | `skills` | A widely used community collection of engineering skills, under the MIT licence. It ships for Claude Code, Codex and opencode alike. |

**An entry comes off the list** in a pull request of its own once it stops meeting these rules: when it is no longer maintained, or when the repository changes hands. A new owner is someone the list never chose. Skills already installed from it keep their pins; only the offer goes.

Being on the list vouches for nothing. The review is what stands for consent (§7), and it applies to these sources as much as to any other.

*Rejected:* a longer list of popular repositories. Each one more is a supply-chain claim purlis would seem to make, and a chat program vendor's own collection plus one widely used, cross-program collection is enough to start from. Another source is still one URL away in Add source.

## The first-party catalogue has no source yet

PE-44's first-party catalogue would be a skill source purlis publishes. No such repository exists in October 2026, and opening a public repository is the operator's call, so the list holds no `catalogue` entry. The shape is in place: a `catalogue` table is drawn first and marked *from purlis*. Once the repository exists, it is one more table in the list. The skills purlis ships today are its built-in skills. Every chat already gets them, and the studio lists them, so they are not offered again as a source.

## The move SK-21 left for here

A project whose skills are still in `.claude/skills`, and whose manifest does not yet require `project-skills`, is offered *Move them* in the studio. That is the doctor's `project-skills` fix (#2784), run through the window's fix command, with its refusals and its report shown as the fix says them. The studio counts only the folders there that hold a `SKILL.md`, never through a link, and reads at most 1,000 entries.
