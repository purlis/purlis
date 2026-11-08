# A skill named bare beside the word skill is read as a hint

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, ticket #4510. It closes the last item [a chat is told to load a skill its harness does not know](2026-11-01-a-chat-is-told-to-load-a-skill-its-harness-does-not-know.md) left open. The other item, a narrowing skill that was not handed, was settled by [ADR 0063's record of 2026-11-06](../0063/2026-11-06-a-narrowing-skill-its-harness-was-not-handed-is-loaded.md).

## The problem

A skill written for every harness names its siblings the way Claude Code's skill tool takes them: `Use the Skill tool with "grilling"`. `purlis skill uses`, the skill lint and an install's review read only `skills:<name>`, so such a skill looked like it named nothing. The person was never told it depends on `grilling`, and an install of it alone left `grilling` behind.

## The decision

**A bare name is a hint where all four of these hold** (`purlis_skills::uses::hints_in`):

1. It is the whole of a quote closed on its line: straight double quotes, curly double quotes, or a code span's backticks. Single quotes are not read, since they are apostrophes as often.
2. The rest of that line holds the word "skill" or "skills" on its own, in any case ("Skill tool", "the skills"), not inside a longer word such as "skillful".
3. It is a skill's name by the open format's grammar, and names a skill that is there: one of the same project for `skill uses` and the lint, and for an install's review one the same source holds or the level it lands in has. A quoted word that names no skill is never read, so a hint is never broken.
4. It is not the skill's own name, and the skill does not also reference it as `skills:<name>`. Where it does, it is a reference.

**Each reader marks a hint as one:**

- `purlis skill uses` prints `uses "grilling"  (a hint: named bare, not as skills:grilling)`, and on the other end `hinted at by <skill>  (named bare)`. `--json` gives each use a `hint` field, plus a `hinted_by` list beside `used_by`.
- The lint warns once per hinted skill that purlis only guesses which skill is meant, and says to write `skills:<name>` so purlis checks it and keeps it right on a rename. A warning never stops a save.
- An install's or update's review brings a hinted skill the source holds along, as it does one it refers to, and leaves one the level has as it is. Each review lists its `hints` apart from `refers`, and a skill that comes along only by a hint is marked `brought_by_hint`. The terminal says `(comes along: <skill> names it bare, read as a hint)`.

**Nothing decides what a chat does from a hint.** A hint is what purlis tells the person, and a skill an install lists in the review the person accepts or declines as a whole.

## Rejected

- **Any bare word that matches a skill's name.** Words like `review` or `deploy` are everywhere in prose. Requiring a quote and the word "skill" on the same line keeps this to the way skills actually name each other.
- **Single quotes.** `it's` and `the team's` would open a quote.
- **A hint the lint calls broken.** Only names of skills that are there are read, so a typo in a bare name is not caught. A `skills:<name>` reference is.
- **Following hints in a publish.** A publish still sends the skills a skill references, and no others. The person reads a hint in `skill uses` and the lint before publishing.

## What remains

The window does not show hints yet. The skill's tab draws references only. The window's install review brings a hinted skill along, as the terminal's does, but labels it "Comes along: X refers to it". That is loosely true, since X does name it, and it is accepted as debt until a hint has a mark of its own there. Holding hints back from the window's install alone would make the two installs of one source differ. Marking a hint in the window needs a prototype first (#5874).
