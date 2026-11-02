# Amendment, 2026-11-01 (#4480): the steps family

Pool setup (spec #4469) needs a guide's list of steps, each with its state, and nothing in the
window drew one. The prototype on `prototype/pool-setup` hand-drew it, and the person ruled that it
becomes a kit part as it is needed, with stories as the record.

- **The steps family** (`app/src/ui/steps/`, on React Aria's `Button`): `StepList`. A step's variant
  is its state: `done`, `current`, `attention`, `skipped`, `waiting` and `later`, each told apart by
  its mark and its words as well as its colour (WCAG 1.4.1). A `later` step can't be pressed: a
  person jumps back, never ahead. Its `data-scope` by hand is `purlis/steps-family`, an error.
