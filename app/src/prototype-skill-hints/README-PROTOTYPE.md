# PROTOTYPE: the window marks a skill named bare as a hint (#5874)

Throwaway. It lives on branch `prototype/skill-hints` and never merges to main.

## The question

How does a hint (a skill named bare beside the word skill, ADR 0102 as amended 2026-11-08) look
next to a reference on the skill's tab, does the tab offer the fix (`skills:<name>`), and how does
the install review word "comes along by a hint", and can the person untick it?

## Open it

- Storybook: `cd app && npm run storybook`, then **PROTOTYPE / Skill hints**.
- One page: `cd app && SP_OUT=$TMPDIR/hints npx vite build --config src/prototype-skill-hints/vite.standalone.config.ts`,
  then open `$TMPDIR/hints/standalone.html`.

`?variant=A|B|C` and `?moment=tab|review`; ← → switch variants. Stand-in data.

## Variants

- **A, a mark on the row.** Hint badge, why under it, "Write skills:<name>" on the row; the review
  says "Comes along by a hint" and lets you untick it.
- **B, its own group.** "Refers to" / "Hints at", one fix for the group; the review lists hinted
  skills apart, unticked.
- **C, quiet, in a tooltip.** "named bare" with the fix in its tooltip, no action on the tab; the
  review locks it like a reference.

## Verdict

Not seen yet.
