# The skill budget's numbers, manual-only skills and Use skill…, as #2775 built them

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-21**, ticket #2775 (SK-12). ADR 0102 §1 says a chat's skill budget is purlis's, in skills and description characters, set below each harness's own limit, and shown on the chat tab. It names no numbers and no threshold. This records the ones #2775 chose, what counts against them, and how Use skill… starts a skill.

## The budget is 30 skills and 6,000 description characters, on every harness

| Harness | Its own limit (measured 2026-10-20) | purlis's budget |
|---|---|---|
| Claude Code 2.1.304 | 1% of the context window, about 8,000 characters on a 200k-token window, for names, descriptions and the lines they sit on; each description cut at 1,536 characters; descriptions dropped past it | 30 skills, 6,000 characters, each description counted up to 1,536 |
| Codex 0.163.0 | 2% of the context window, 8,000 characters where the window is not known; each description cut at 1,024; trailing skills dropped silently past it | 30 skills, 6,000 characters, each description counted up to 1,024 |
| opencode 1.18.33 | none: every skill goes into the system prompt | 30 skills, 6,000 characters, nothing cut |

- **One number for every harness.** The smallest limit is Claude Code's, about 8,000 characters in all. 6,000 characters of descriptions leaves room for 30 names and the lines they sit on, so the listing purlis hands any harness never reaches its own cut. A per-harness number would let the same project warn on one harness and not another, for one set of skills.
- **The threshold is the budget itself.** A chat whose set is over either number warns, on its pane's Skills chip and in Use skill…, saying which part is over. There is no "near" warning: the budget is already below every harness's cut, so being at it costs nothing.
- **The numbers are each adapter's to say** (`HarnessAdapter::skill_budget`), with the shared 30 and 6,000 in one constant and each adapter's own cap on one description. A harness whose limit changes changes its adapter.

## What counts

- Every skill of the chat's set costs one skill and its `description`'s characters, up to its harness's cap on one. Characters, not bytes.
- **A manual-only skill costs nothing.** Manual-only is the standard frontmatter key `disable-model-invocation: true` (`yes`, `on` and `1` too): no harness picks the skill by itself, and its description stays out of the listing. It is still listed in Use skill….
- The router's pins and index skill (§2, §4) are not built yet. When they are, they count first, as §4 says.

## Use skill… starts a skill in the chat's harness's syntax, never sent

There is no chat composer: a chat is its harness's own terminal. So Use skill… is a palette row for the chat in front and a Skills chip in each chat pane's top line, which also shows the budget. Both list exactly that chat's skill set and nothing else.

- A pick types one line into the chat, in its harness's syntax, as a reference is typed: one bracketed paste with nothing after it, only while the chat is at its prompt, and copied to the clipboard with the reason otherwise (opencode, which cannot be typed into, and a chat mid-turn).
  - Claude Code: `/purlis:<name>` for a built-in skill, `/skills:<name>` for one of the plane's, then each argument a shell word.
  - opencode: `/<name>`, then each argument a quoted word.
  - Codex: `$<name>`, then `name: value; …` in plain words, since Codex substitutes no arguments.
- **Arguments are the standard frontmatter's** (decision 61): `arguments` names them in order (a space-separated string, a flow list or a block list), and `argument-hint` gives a skill that names none a single line. A small dialog asks for them before anything is typed. purlis adds no key.
- **A skill that was typed has its run start when the prompt it was typed into is sent**, started by `use-skill`. The window mints the run's id as it types, holds it for the chat, and the event log and the chip open it at the chat's next `UserPromptSubmit`. Neither Claude Code's typed `/name` nor opencode's slash path goes through a Skill tool call, so without this they would start no run. A read of the same skill in that turn (Codex) is the same run. The prompt's text is not reported to purlis, so a person who clears the line and sends something else still opens the run; a copied skill opens none.
- The set is worked out now, as the chat's start worked it out. A skill added to the plane after the chat started is listed though the harness has not loaded it; #2770's restart offer is the answer there.
