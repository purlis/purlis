# The built-in router, pins and the index skill, as #2778 built them

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-25**, ticket #2778 (SK-15), first slice. ADR 0102 decides what a router does; it names no place for the index skill, no key for a pin, and does not say which skills a router may hide. This records what the first slice chose. Router choice, extension routers with their 100 ms (§5, §6), the pointer on a prompt (§3) and the routing record are follow-ups (#3230, #3231), as are a pin switch in the studio (#3232) and grading routers (#3233).

## What the built-in router hides, and what it never does

Past the chat's budget (ADR 0102 as amended 2026-10-21: more skills or more characters than it holds, not the 80% warning), the built-in router hides **every plane skill purlis hands by a listing route that is neither pinned nor manual-only**, all of them at once, and hands the index skill in their place. It never hides:

- **A built-in skill.** Every route hands purlis's built-in skills as the one bundled `skills/` folder, whole (ADR 0063); hiding one would mean a second copy of the bundle per chat. They count first, as pins do.
- **A harness or repo skill.** Its harness lists it itself; purlis can only ask the harness to leave it out, which is turning it off, not hiding it.
- **A manual-only skill.** No listing holds it, so it costs nothing (ADR 0102 as amended 2026-10-22), and its harness must still be handed it for Use skill… to start it by its own syntax.
- **A pinned skill** (§4).

It hides all the hideable skills rather than as many as fit, so the listing does not change with the order of names, and a small library past the budget reads the same as a large one: what the person pinned, purlis's own, and the index.

A chat on the fallback route (`Route::Mcp`) lists nothing: every skill is found with `skill_find`. Nothing is hidden there and it has no index.

## Hidden is not left out

A hidden skill stays in the chat's skill set (`SkillSet::effective`), with a `Routing` beside it saying which router hid it and why (`Hidden`, `HiddenWhy::OverBudget`). So `skill_find` and `skill_load` reach it unchanged, the studio lists it as on with its reason ("hidden by the built-in router: over this chat's skill budget, so it is in the index skill"), and Use skill… offers it. What a route hands (`SkillSet::from_the_plane`, the set's folder, a Codex briefing, the restart stamp) leaves it out and carries the index instead.

**Use skill… on a hidden skill types a sentence, not the harness's syntax**: "Load the skill `<name>` with skill_load and follow it.", with any arguments in plain words. Its harness was not handed the skill, so `/skills:<name>` would name nothing. No run is held for it: `skill_load` opens the run with an exact start.

## The index skill

- **Named `skill-index`**, a name purlis keeps on every route whether or not a router hands an index: a plane skill of that name never loads ("purlis's index skill has this name, so this skill never loads: rename it"), and a harness or repo skill of that name is listed as "hidden by purlis's index skill of this name", as one named as a built-in skill is.
- **Laid out in the set's folder** (`<app data>/skill-sets/<digest>/skills/skill-index/SKILL.md`) beside the listed plane skills, as purlis writes every handed `SKILL.md`, through the harness's frontmatter translation. So Claude Code loads it as `skills:skill-index`, opencode scans it, and a Codex briefing names it from the folder. The folder's digest covers it, so a change to what is hidden is a new folder and a running chat's folder never changes.
- **Its text**: a fixed `description` (what it costs against the budget: one skill and its characters, counted when the router checks what is still over), then every hidden skill as `` - `name`: one line ``, deprecated skills last with their replacement. The model reads it, so nothing in it is hidden from a person reading it too:
  - a name is listed only where it is a skill's name by the open format's grammar (`name_problem`: 1 to 64 lowercase letters, digits and hyphens); the others are counted in one closing line that says `skill_find` finds them;
  - a description is folded onto one line, cut at 120 characters, and every character a reader would not see (invisible, bidi and tag characters, by `purlis_unicode`'s one rule) is written as its mark, `⟦U+202E⟧`.
- **A chat's restart stamp includes it**, so a skill added past the budget offers the restart as any added skill does, and the stamp counts the built-in skills, as arming does, so what it says is hidden is what the chat was handed.

## Pins are two keys

- A workspace's: `settings.skills.pinned`, a list of names in its `workspace.json`, beside `settings.skills.off`, refused by the save in any other shape.
- A persona's: `pinned-skills:`, a comma-separated line in its own definition, beside `skills:`. Not inherited along `extends`, as `skills:` is not.
- A chat's pins are both together, each name once. A pin only keeps a skill of the set listed: it never adds one (`skills:` is what brings a skill in). `purlis skill pin <name> -w <ws> | -p <persona> [--off]` writes either as one commit, and refuses a name no skill has.
- **Still over the budget** with all the router may hide hidden, everything is listed, and which of two things is over is said apart (`StillOver`):
  - `NotHideable`: what no pin decides (the built-in skills, the harness's and the repos' own, and the index) is over by itself. Use skill… says so and points at turning some of the tool's or repos' skills off.
  - `Pins`: that fits, and the pins on top of it do not. Use skill… says to unpin some with `purlis skill pin <name> --off`.
- **Pins are held to limits** before anything is done with them, since they are read from files of the project: at most 64 a chat, each a skill's name by the format's grammar. Past either, the chat reads no pins at all and the log says why; a pin only ever keeps a skill listed, so reading none fails closed.

The spec's line "pinned flag kept beside the folder" is not taken: a pin belongs to the workspace or persona that pins, as ADR 0102 §4 says, and one skill pinned by one workspace is not pinned by another.

## Where it is worked out

`purlis_skills::SkillSet::routed(budget, pins)` is the one place. purlis-core routes where a chat is armed (`for_chat`, `lay_out_for_chat`), for its restart stamp, for Use skill… and for the studio, and never for the set `skill_find` and `skill_load` answer from, which is the whole set. **The studio routes each harness's set by that harness's own budget**, with the skills that harness finds itself counted, and a row says on which harnesses its skill is hidden: Codex counts each skill's path, so a library can be past its budget there and not on Claude Code or opencode.
