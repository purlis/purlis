# PROTOTYPE: the Skill studio shows an org's [skill_policy] controls (#6031)

Throwaway. It lives on branch `prototype/skill-policy` and never merges to main.

## The question

Where does the window show a project's `[skill_policy]` (allow, block, `min_age_days`,
`min_age_exempt`) and its effect: what Sources would refuse before a review, an update too young
to take and when it can be, and the audited installs, updates and removals?

## Open it

- Storybook: `cd app && npm run storybook`, then **PROTOTYPE / Skill controls**.
- One page: `cd app && SP_OUT=$TMPDIR/policy npx vite build --config src/prototype-skill-policy/vite.standalone.config.ts`,
  then open `$TMPDIR/policy/standalone.html`.

`?variant=A|B|C` and `?moment=sources|updates|settings|record`; ← → switch variants. Stand-in
data; the refusal sentences are the core's (`skills/source/policy.rs`).

## Variants

- **A, at the point of refusal.** Each row says what the controls do to it; one Notice on Sources;
  Settings holds the read-only table and the recorded acts.
- **B, a Controls page.** A studio page of its own: rules, what they refuse now, what waits to be
  old enough, recorded acts. Rows carry a Refused mark that opens it.
- **C, only what is allowed.** Sources and Updates list what is admitted; the rest folds under one
  line; Settings holds the table and the record.

## Verdict

Not seen yet.
