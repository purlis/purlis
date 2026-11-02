---
name: purlis-building-purlis-ui
description: Build or change purlis's own window — its screens, dialogs and component families in a purlis/purlis checkout's `app/src` — from the families in `app/src/ui`, with stories, axe, keyboard tests and screenshot baselines, until every check is green. Use when asked to add, change, fix or restyle a part of the purlis app's UI, or a component family and its theme parts.
---

# Building purlis's UI

The purlis window is built from **component families** (ADR 0104): each kind of control is one
family in `app/src/ui/<family>`, wrapping its React Aria primitive once, drawn by its **parts**
(`data-scope`, `data-part`, `data-variant`) and themed through the parts manifest (ADR 0103). A
screen composes families; the look is the family's and the theme's. This skill applies only in a
checkout of the purlis repository: every path below is from its root.

The docs are the source of truth, and this skill points into them. Read the section a step names
before you write the code it covers:

- `docs/ui-primitives.md`: what the window is built from, family by family, with each family's
  props, keyboard and rules.
- `docs/design-system.md`: what it is drawn in: tokens, geometry, motion, text roles and the
  guards.
- `docs/component-stories.md`: what a story is held to, and when a family is done.
- `crates/purlis-core/docs/themes.md`: the part grammar, bounds and floors a theme meets.
- `docs/ui-copy.md`: the words on every control.

## 1. Prototype a design change first

A change to what a screen looks like or how it is laid out is drawn as a **prototype** before it
is built: a standalone `app/src/<Screen>.prototype.html` on a `prototype/<name>` branch, never
merged (`prototype/switcher`, `prototype/start-chat-dialog`, `prototype/settings-page` are the
precedents). The operator looks at it and rules. A fix that keeps today's look needs none.

Once a prototype is approved, **build it 1:1** (operator ruling on spec #2897): every
prototyped size, gap, colour role and behaviour, nothing lost. Where an ADR, a later ruling or the
deviations in the pinned [prototype audit](https://github.com/purlis/purlis/blob/f4bb15744991dc9482b52dc2741f50b66e871425/docs/research/prototype-audit.md)
(§7) differ from the prototype, build to the ruling and record it as a D-line in the pull request.

Done when: the design change has an approved prototype, or it changes no look.

## 2. Compose the families

Draw every control from its family: `Button`, `IconButton`, `ToggleButton`, the field, select,
combobox and choice families, `Dialog` and `AlertDialog`, menu, popover, tooltip, list, tabs,
panel, empty state, status item, rail, activity and title bar. `ls app/src/ui` is the current
set; `docs/ui-primitives.md#every-control-is-a-familys` says which control is which family's.

- A screen imports the family, never `react-aria-components`, `react-aria` or `react-stately`.
- A family's props say the variant, the size and the state. A screen's `className` places the
  control (margin, width, flex, order) and never draws it.
- The lint rules in `app/eslint.config.js` (`purlis/<family>-family`, from
  `app/eslint.ui-rules.js`) refuse a hand-drawn control of every family that has moved, at error.
  A lint error is the answer: draw it from the family. A raw `<button>` passes only as a control
  of a family still to move, said by its role or class in `OTHER_FAMILIES`.
- Something no family draws yet is that family's ticket: build it in `app/src/ui`, under the
  family definition of done (step 5), rather than inline in a screen.

Hold the window's rules while composing:

- **Nothing shifts.** A state changes a colour, a fill, a shadow or a ring, never a size, so
  nothing moves under the pointer, a press, the keyboard or a choice. Text that changes with a
  choice reserves its room (`Reserved`, `reserve`).
- **Dialogs** follow the dialog rules: header, scrolling body, footer always in view, a width by
  size, a fixed top, at most three answers in their places, ⏎ and Esc
  (`docs/ui-primitives.md#the-dialog-family`).
- **Motion** reads a duration token and only explains a change: a part animates as it appears,
  never because it mounted, never as it closes, and reduced motion is the theme layer's job
  (`docs/design-system.md#what-moves-and-what-is-deliberately-still`).
- **Colour and geometry** are tokens only: a literal colour outside `app/src/theme/`, or a
  literal padding, gap, corner or font size off the scale, fails the guards
  (`docs/design-system.md#the-guards-and-the-lists-that-only-shrink`).

Done when: every control on the screen is a family's and lint is clean.

## 3. Hold accessibility and consent

purlis meets WCAG 2.2 AA, held in CI:

- **Contrast floors**: words 4.5:1, icons, edges that are a part's only edge, focus rings and
  indicators 3:1, on every ground the part is drawn on. The token floors are
  `app/src/theme/contrast.test.ts`'s; the engine holds every part and state to them (ADR 0103 §7).
- **Targets** at least 24px (2.5.8), the **focus** ring from the keyboard only and **never under a
  sticky footer** or anything drawn over it (2.4.11), every path reachable by keyboard.
- **Consent marks** hold under every theme (ADR 0097 §6, ADR 0103 §7): an answer that grants is
  `variant="grants"`, with its lock and accent edge, as plain as Keep blocked beside it; Allow
  never takes the primary look and never takes the keyboard as a dialog opens
  (`app/src/grantFocus.guard.test.ts`); danger stays apart from primary. A refused value is the `invalid`
  state, whose edge no theme erases.

Done when: the screen's stories pass axe and their keyboard paths (step 5).

## 4. Add parts and theme keys

When a family gains a part, a variant or a property, the manifest says it once:
`crates/purlis-core/src/theme/parts_<family>.rs`, with its tests in `parts_<family>_tests.rs`, and
its default look in `app/src/ui/<family>/<family>.css`, keyed on the wrapper's attributes. A key
is `<family>.<part>[.<variant>][.<state>].<property>`.

**The states are a closed list**: `hover`, `pressed`, `selected`, `focused`, `disabled`, `open`
and `invalid` (rest is implied). A family names only the states its wrapper marks; `invalid` is
for a box a person gives a value to. Inside a state a theme sets only fill, text, border colour
and shadow: a state never changes size.

**Mark every addition `since: NEXT`**: `super::NEXT` in the family's own file, `parts::NEXT` in a
test, never imported and never a literal number. Leave `parts::VERSION` and the digest table
alone; the release numbers them. Then write again what is generated from the manifest, never by
hand: the schema, the docs family table and, for a built-in theme, the stories' copy
(`docs/component-stories.md#when-a-component-family-is-done` has each command). On a conflict in
one of them, take main's and write it again.

Done when: `purlis theme parts` lists what you added, and the parts tests pass.

## 5. Write the stories

Every component the change draws has stories in `<Component>.stories.tsx` beside it, as
`docs/component-stories.md#writing-one` says:

- a story for each variant and state that draws differently, in every cell of the theme matrix;
- a `play` function on every interactive story walking its keyboard path, Esc where it
  dismisses, with `expectFocusInSight()` after each move;
- axe at WCAG 2.2 AA with `target-size` on; a rule turned off only on one story, with its reason;
- a screenshot baseline per pictured cell, drawn only by the pull request's `stories` run.

**Baselines come from CI, never a local run.** Push; when only pictures fail, the same run draws
the baselines. Take them with `node tools/story-baselines-pull.mjs <pull request>` in a checkout
of the head, review them, and push them with your next change, or put the `update-baselines` label
on (`docs/component-stories.md#baselines`). Delete the baselines of a story you rename or remove,
in the same change.

Done when: every gate passes in Chromium and WebKit and the pictures are committed and reviewed.

## 6. Run the checks

From `app/`, before pushing:

```bash
npm run typecheck
npm run lint
npm run format:check
npx vitest run --project unit <the test files your change touches>   # guards included
npm run test:stories                       # stories, keyboard and axe; pictures only in CI
```

For a parts change, from the root: `cargo test -p purlis-core --lib theme::parts`. CI runs the
rest, the WebdriverIO scenarios and every picture; a red annotation names the story, browser,
cell and gate.

## 7. Done means

UI work is done when every check above is green on the pull request and:

- every control is a family's, with lint at error for each family the change moved;
- each screen a prototype drew matches it on screen, deviations recorded as D-lines;
- the docs say what changed (`docs/ui-primitives.md` for a family, `themes.md` for its parts);
- the operator has had the before and after look at every screen it changes.
