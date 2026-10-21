# Component stories

**A component is held to what a person sees and does by its stories.** Each story is a test:
it renders in real Chromium and real WebKit, its `play` function walks it with the keyboard, axe
checks it against WCAG 2.2 AA, and a picture of it is compared with a baseline. CI fails on any
of the four (#2901, part of the UI system spec #2897).

Its siblings: [`ui-primitives.md`](ui-primitives.md) says what the window is built out of,
[`design-system.md`](design-system.md) what it is drawn in.

## What a story is held to

| Gate | What fails it | Where it is set |
| --- | --- | --- |
| **Renders** | The story throws, or its component does, in either browser. | `app/.storybook/vitest.project.ts` |
| **Keyboard** | Its `play` function fails: a key that does not move the focus or the value it should, Esc that does not dismiss, a focus that ends up out of sight. | the story's `play` |
| **Accessibility** | Any axe violation of a rule tagged `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa` or `wcag22aa`, with `target-size` (WCAG 2.5.8) switched on, which axe leaves off by default. | `app/.storybook/preview.tsx` |
| **Screenshot** | Its picture is not its baseline, or it has no baseline. | `app/.storybook/screenshots.setup.ts` |

Both browsers run every story: WebKit is the engine purlis ships in on macOS and Linux
(WKWebView, WebKitGTK), and Chromium is WebView2's on Windows. A picture is kept for each.

A good story asserts what a person sees or does: a role and its name, a state, the keys, the
pixels. It never reaches into a component's internals.

## Writing one

A story lives beside its component, in `<Component>.stories.tsx` under `app/src`, as Component
Story Format 3. `app/src/settings/Choice.stories.tsx` is the first, and the one to copy:

- Draw the component with the props a real call site hands it. A controlled component gets a
  small wrapper that holds its value, so the keyboard can change it.
- Every interactive story has a `play` function for its keyboard path: Tab to it, every key it
  answers to, Esc where it dismisses something, and Tab or Shift+Tab out again. Wait for what a
  key causes with `waitFor` rather than assuming it is synchronous.
- After each move, `expectFocusInSight()` from `app/src/stories.testkit.ts` checks that the focus
  is on screen and not under anything the page draws over it, such as a dialog's sticky footer
  (WCAG 2.4.11).
- The story is drawn in a built-in theme, the toolbar's **Theme** global; a story that must be
  drawn in another sets `globals: { theme: "<name>" }`.
- The picture is of the story's frame. A story that draws outside it (a dialog, a menu, a
  popover) sets `parameters: { screenshot: "page" }`; one with nothing stable to draw sets
  `screenshot: false` and says why.
- A rule axe must not run on a story is turned off on that story alone, in
  `parameters.a11y.config.rules`, with the reason beside it. Never turn a rule off for every
  story.

## Running them

```bash
cd app
npx playwright install chromium webkit   # once: the browsers Vitest drives
npm run test:stories                     # every story, both browsers, keyboard and axe
npm run storybook                        # the stories in Storybook, on http://localhost:6006
```

`npm test` runs only the unit tests (jsdom); `npm run test:stories` runs only the stories.

**Pictures are compared only in CI.** A picture depends on the machine's fonts and rasteriser, so
on any machine but the pinned image it would differ from its baseline without anything being
wrong. A local run checks rendering, the keyboard and axe, and takes no picture.

## Baselines

A baseline is a PNG at
`app/src/<dir>/__screenshots__/<Component>.stories.tsx/<story id>-<chromium|webkit>-linux.png`.
Every baseline is drawn by one image, Playwright's own Docker image for the Playwright version
the lockfile names (`mcr.microsoft.com/playwright:v<version>-noble`), and compared in the same
image by the `stories` workflow (`.github/workflows/stories.yml`).

**A baseline changes only through the `update-baselines` label.** To add or change one:

1. Push the change that adds the story or changes how it draws. Its `stories` check fails,
   naming the picture that is new or differs; the pictures that differ are kept on the run as
   `story-pictures`.
2. Put the `update-baselines` label on the pull request. `.github/workflows/stories-baselines.yml`
   runs every story in the pinned image, writes each picture that is new or differs, pushes them
   to the branch as one commit, takes the label off and comments which baselines changed.
3. Review the pictures in the pull request's diff, which shows each one beside the one it
   replaces.
4. The push starts the `stories` check on the new commit. The other checks run on the branch's
   next push (or `gh pr update-branch`).

The `stories` check also refuses a baseline the pinned image did not draw (one named for another
platform), and a baseline whose story file is gone (`tools/story-baselines.mjs check`). When a
story is renamed or deleted, delete its baselines in the same change.

Only a pull request from a branch of this repository can be labelled this way: the workflow's
token cannot push to a fork.

When the lockfile's Playwright moves, the `stories` check fails until both workflows name the
image of the new version, and every baseline is drawn again with the label.
