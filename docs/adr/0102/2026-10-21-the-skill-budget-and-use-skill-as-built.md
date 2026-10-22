# The skill budget's numbers, manual-only skills and Use skill…, as #2775 built them

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-21**, ticket #2775 (SK-12). ADR 0102 §1 says a chat's skill budget is purlis's, in skills and description characters, set below each harness's own limit, and shown on the chat tab. It names no numbers and no threshold. This records the ones #2775 chose, what counts against them, and how Use skill… starts a skill.

## The budget is 30 skills and 6,000 description characters, on every harness

| Harness | Its own limit (measured 2026-10-20) | purlis's budget, and what counts |
|---|---|---|
| Claude Code 2.1.304 | 1% of the context window, about 8,000 characters on a 200k-token window, for names, descriptions and the lines they sit on; each description cut at 1,536 characters; descriptions dropped past it | 30 skills, 6,000 characters; each description counted up to 1,536; a manual-only skill costs nothing |
| Codex 0.163.0 | 2% of the context window, 8,000 characters where the window is not known; each description cut at 1,024; trailing skills dropped silently past it | 30 skills, 6,000 characters; each description counted up to 1,024, with the `SKILL.md` path on its briefing line; a manual-only skill counted |
| opencode 1.18.33 | none: every skill goes into the system prompt | 30 skills, 6,000 characters, nothing cut; a manual-only skill counted |

- **One number for every harness.** The smallest limit is Claude Code's, about 8,000 characters for names, descriptions and their lines together. 6,000 characters of descriptions leaves room for 30 names and the lines they sit on, so the listing purlis hands any harness stays under its own cut. A per-harness number would let the same project warn on one harness and not another for one set of skills.
- **Codex counts the path too.** A Codex chat is briefed on its skills, each line naming its `SKILL.md`'s path beside its description (ADR 0063), and that path is in its 2% as the description is. Its budget counts each path's characters, so the briefing stays within the same 6,000.
- **The warning comes early: at 80% of either number.** A chat whose set reaches four fifths of its skills or of its characters warns, on its pane's Skills chip, on its tab's Use skill… row and in Use skill…, saying which part. The router switches to the index only past the budget itself (§2), so the person hears of it before any skill is hidden.
- **The numbers are each adapter's to say** (`HarnessAdapter::skill_budget`): the shared 30 and 6,000 in one constant, and each adapter's cap on one description, whether its listing carries paths, and whether it leaves a manual-only skill out. A harness whose limit changes changes its adapter.

## What counts

- Every skill of the chat's set costs one skill and its `description`'s characters, up to its harness's cap on one, and its path where the harness lists it. Characters, not bytes.
- **A manual-only skill costs nothing only where the harness keeps its description out.** Manual-only is the standard frontmatter key `disable-model-invocation: true` (`yes`, `on` and `1` too). Claude Code reads it and leaves the description out, so there it is free. Codex is briefed on every skill, and opencode 1.18.33 reads no such field, so on both it is still listed and is counted, and the budget says "still listed on this chat", until SK-8 (#2771) hands each the translation. It is offered by Use skill… on every harness.
- The router's pins and index skill (§2, §4) are not built yet. When they are, they count first, as §4 says.

## Use skill… types a skill in the chat's harness's syntax, never sent

There is no chat composer: a chat is its harness's own terminal. So Use skill… is a palette row for the chat in front, a row on each chat tab's menu, and a Skills chip on each chat pane's top line, which shows the budget. The chip gives way beside a task's breadcrumb and in a pane 40rem wide or less, where the line's state needs the room; the tab's row then says the budget. All three list exactly that chat's skill set.

- A pick types one line into the chat, in its harness's syntax, as a reference is typed: one bracketed paste with nothing after it, only while the chat is at its prompt (its hooks said it waits, and its terminal is raw), and copied to the clipboard with the reason otherwise. **opencode is typed into too**: what keeps purlis from typing a first prompt into it is the race at its start, which a chat whose hooks have said it waits has passed.
  - Claude Code: `/purlis:<name>` for a built-in skill, `/skills:<name>` for one of the plane's, then each argument a shell word.
  - opencode: `/<name>`, then each argument a quoted word.
  - Codex: `$<name>`, then `name: value; …` in plain words, since Codex substitutes no arguments.
- **Arguments are the standard frontmatter's** (decision 61): `arguments` names them in order (a space-separated string, a flow list or a block list), and `argument-hint` gives a skill that names none a single line. A small dialog asks for them before anything is typed. purlis adds no key.

## A typed skill's run starts only once the chat shows it was sent

The window mints a run's id as it types and holds it for the chat, beside any other typed before it (each pick is its own paste). It opens, started by `use-skill`, only on the chat's own word:

- **Claude Code names it.** A `/<name>` the person sends fires `UserPromptExpansion` with its `command_name`. purlis wires that hook (`userpromptexpansion`, answering nothing), and the held run of the skill of that name opens with the prompt it was sent in, whichever of the two is heard first. A turn that ends without naming it drops it.
- **Codex and opencode, by the prompt.** The next prompt opens the run where a line of it holds the typed text, and drops it either way. The prompt never leaves its hook: the hook sends a 256-bit filter of the digests of the stretches of each line (from any word, up to 6 words, at most 36: `prompt_marks`), which only a live host hears, never the spool or the event log. A filter answers yes now and then for text it does not hold; a typed skill whose text is longer than a stretch, or sits past the stretches marked, is not found, and its run does not open.
- **One held ten minutes is dropped**, and so is every one held for a chat whose session ends. A copied skill is held nowhere.
- A read of the same skill in that turn (Codex) is the same run.

The set is worked out now, as the chat's start worked it out. A skill added after the chat started is listed though the harness has not loaded it; #2770's restart offer is the answer there.
