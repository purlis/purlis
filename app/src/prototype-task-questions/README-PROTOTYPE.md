# PROTOTYPE: the question surfaces (spec #4551, ticket #4561)

Throwaway. It lives on branch `prototype/task-question-surfaces` and never merges to main. The
person reviews it before the tickets that build these surfaces start (ruling 2026-10-28).

## The question

Do the waiting strip, top-tab quiz, task row states, first-time line and question trail hold the
spec's visibility rule and smoothness checks, in every state it names? ADR 0107 and the spec fix
the design, so this draws one design in full. The one place the spec's words can't be drawn as
written gets a switch: Band or Foot.

## Open it

One command, from any clone of purlis, with nothing to install or build:

```bash
git fetch origin prototype/task-question-surfaces && git show FETCH_HEAD:app/src/prototype-task-questions/PROTOTYPE-artifact.html > "${TMPDIR:-/tmp}/task-questions.html" && open "${TMPDIR:-/tmp}/task-questions.html"
```

The dashed bar at the bottom is the prototype's, not the design. `[` and `]` go through the states,
`\` lists them, and the bar switches Band or Foot and Light or Dark. Where a state waits on an agent,
a button on the bar plays that event. The real buttons work: answer a quiz, press ✋, change the
setting, open a task. Stand-in data, in memory. Nothing calls the core.

## What is here

- `CHECKS.md`: the page measured against the visibility rule and the smoothness checks, and the
  decisions for the person.
- `screenshots/`: every state in dark, numbered in the order of the list; `light-*` in light;
  `foot-*` with Foot; `bar*.png` with the prototype's bar.
- `copy.ts`: every word. The page's last state, Every word, lists them.
- `model.ts`: one world in memory with every state; every surface is computed from it, so no surface
  holds a copy. `PrototypeQuestions.tsx` draws it from `app/src/ui` and the Inbox's own classes.
- `capture.mjs`: draws the screenshots and runs the checks into `checks.json`.

The terminal, the tab strip and the title bar are stand-ins. The prompt is a text box where a real
pane has the harness's own input.

## Rebuild

From `app/`:

```bash
npx vite build --config src/prototype-task-questions/vite.standalone.config.ts
cp node_modules/.tq-dist/standalone.html src/prototype-task-questions/PROTOTYPE-artifact.html
node src/prototype-task-questions/capture.mjs
```

In a sandboxed chat, set `TQ_CHROMIUM` to an installed headless Chromium and
`TQ_ARGS="--single-process --no-zygote"`: Seatbelt refuses Chromium's child-process port.

## Verdict

Waiting on the person's review (#4561).
