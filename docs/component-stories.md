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

## The theme matrix

**Every story runs in every cell of the matrix** (#2945): each appearance of each theme purlis
ships, in each density. One decorator in `app/.storybook/preview.tsx` draws it in its cell, so no
story picks a theme of its own; each cell is a Vitest project of its own, `stories <cell>
(<browser>)` (`app/.storybook/vitest.project.ts`, `app/.storybook/matrix.ts`).

| Theme | Appearances | Densities | Pictures |
| --- | --- | --- | --- |
| Purlis Classic (`purlis-classic`) | `dark`, `light` | `comfortable`, `compact` | dark and light, comfortable |
| Purlis (`purlis`) | `dark`, `light`, `dark-high-contrast` | `comfortable`, `compact` | dark, light and dark high contrast, comfortable |

Every cell renders the story, plays its keyboard path and runs axe. A picture is taken only in
the cells `app/.storybook/pictured.json` names, each its own baseline, named for its theme and
appearance: `<story id>_<theme>_<appearance>-<browser>-linux.png`, so
`ui-button-primary_purlis_light-webkit-linux.png` is the Primary button in Purlis's light
appearance in WebKit. A cell's name is `<theme>_<appearance>`, and `_compact` after it for the
compact density, which its pictures' names carry too
(`ui-button-primary_purlis_dark_compact-webkit-linux.png`). Any cell can be pictured by adding its
name to `pictured.json`; five are today (Purlis's high contrast since #2924, so its derived
look is seen beside dark and light), to keep the `stories` job short, and axe and the
keyboard run in all ten. A picture's name is held to the names `tools/story-baselines.mjs`
takes, cell by cell, in `tools/story-baselines.test.mjs`.

Every face a theme names is made before its story renders, so the keyboard path, axe and the
picture all meet the theme's own fonts. A face the theme names that its folder no longer has
stops Storybook and the stories from loading, rather than drawing the fallback.

**The themes are the core's.** A story's page cannot ask the core for anything, so what the core
hands a window for each built-in appearance is kept in `app/.storybook/built-in-themes.json`, and
a test of the app's crate (`app/src-tauri/src/storythemes.rs`) fails when it is not what the core
resolves now. Beside it, `app/.storybook/at-creation.json` names, by theme and appearance, what a
window is handed as it is created: the default's appearance, Purlis's dark (#3034), and each
built-in pick's. A unit test's window and a story's page are handed those, and the toolbar
starts on the default's cell; the same test holds the file to the core, so no story file spells
a default. After a change to a built-in theme, write both again and commit them:

```bash
mkdir -p app/dist && PURLIS_WRITE_STORY_THEMES=1 cargo test -p purlis-app --lib storythemes
```

Each theme's fonts are drawn from the files its folder ships (`app/.storybook/handed.plugin.ts`),
by the digest the window asks for them by, and every face is made before a picture is taken.

In Storybook's own UI the toolbar's **Theme**, **Appearance** and **Density** pick the cell.

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
- The story is drawn in every cell of the theme matrix (below): never pin a theme on a story.
- The picture is of the story's frame. A story that draws outside it (a dialog, a menu, a
  popover) sets `parameters: { screenshot: "page" }`; one with nothing stable to draw sets
  `screenshot: false` and says why.
- A rule axe must not run on a story is turned off on that story alone, in
  `parameters.a11y.config.rules`, with the reason beside it. Never turn a rule off for every
  story.

## When a component family is done

A family's ticket (spec #2897) is done when it ships:

- a story for each variant and state that draws differently, each passing all four gates in both
  browsers;
- a `play` function on every interactive story: its keyboard path, Esc dismissal where it
  dismisses, and the focus in sight after every move;
- its baselines, drawn by the label and reviewed in the pull request;
- its parts in the theme engine's manifest, and its docs;
- every call site moved to it, with the lint rule for it at error;
- the operator's before and after look at the screens it changes.

## Running them

```bash
cd app
npx playwright install chromium webkit   # once: the browsers Vitest drives
npm run test:stories                     # every story, every cell, both browsers, keyboard and axe
npx vitest run --project "stories purlis_*"   # one theme's cells
npm run storybook                        # the stories in Storybook, on http://localhost:6006
```

`npm test` runs only the unit tests (jsdom); `npm run test:stories` runs only the stories.

**Pictures are compared only in CI.** A picture depends on the machine's fonts and rasteriser, so
on any machine but the pinned image it would differ from its baseline without anything being
wrong. A local run checks rendering, the keyboard and axe, and takes no picture.

**Chromium draws its words greyscale** in every story run (`--disable-lcd-text`,
`app/.storybook/vitest.project.ts`). Its subpixel text takes each glyph from a cache the whole
run shares, so a picture could differ by a pixel with the stories that ran before it; greyscale
text draws the same each time.

**What a failure says.** In CI, each story that fails is one annotation on the `stories` check,
titled `<story> in <browser>, <cell>`: its file, the gate it failed (`renders`, `keyboard`,
`accessibility` or `screenshot`), the first line of why, and the axe rules it broke. It says the
same in WebKit as in Chromium: `tools/story-failures.mjs` is the Vitest reporter that keeps each
error's message, and the script that turns the reports into annotations. An error in a story
file or a suite outside any one story is an annotation of its own, and a run that fails without
naming anything says so.

## Baselines

A baseline is a PNG at
`app/src/<dir>/__screenshots__/<Component>.stories.tsx/<story id>_<theme>_<appearance>-<chromium|webkit>-linux.png`,
one for each pictured cell of the matrix.
Every baseline is drawn by one image, Playwright's own Docker image for the Playwright version
the lockfile names, and compared in the same image by the `stories` workflow
(`.github/workflows/stories.yml`). Both workflows name it by its tag and its digest,
`mcr.microsoft.com/playwright:v<version>-noble@sha256:<digest>`, so a tag pushed again never
changes a picture; the `stories` check fails unless both name the lockfile's version and one
digest (`tools/story-image.mjs`).

**A baseline changes only through the `update-baselines` label.** To add or change one:

1. Push the change that adds the story or changes how it draws. Its `stories` check fails,
   naming the story, the browser and the picture that is new or differs; the pictures that differ are kept on the run as
   `story-pictures`.
2. Bring the branch up to date with its base (`gh pr update-branch`), then put the
   `update-baselines` label on the pull request. `.github/workflows/stories-baselines.yml`
   refuses a branch behind its base, runs every story in the pinned image, writes each picture
   that is new or differs, deletes every baseline whose story is gone, pushes both to the branch
   as one commit, takes the label off and comments which baselines it drew and which it deleted.
   What it commits is only whole PNG baselines within a size and a count limit, and the deletion
   of other files in `__screenshots__` directories, checked by the base branch's own copy of
   `tools/story-baselines.mjs`.
3. Review the pictures in the pull request's diff, which shows each one beside the one it
   replaces.
4. The job starts the `stories` check on the new commit itself. The other checks of a push made
   with the workflow's token wait for a maintainer to approve them in the pull request's Checks,
   or run on the branch's next push (or `gh pr update-branch`).

**Which `stories` run counts.** The pull request's own run, on the branch merged into its base,
is the one branch protection reads. The run the label's job starts tests the branch itself; on a
branch that is up to date with its base, which the job requires, the two test the same tree.

The `stories` check also refuses a baseline the pinned image did not draw (one named for another
platform), and an obsolete one: a baseline whose story file is gone, whose story is no longer
in its file with the `test` tag, the stories the Vitest addon runs, or whose cell is not one
`pictured.json` names. Storybook's own index
(`storybook index`) says which stories there are, and the check names the story of each obsolete
baseline (`tools/story-baselines.mjs check`). When a story is renamed or deleted, delete its
baselines in the same change, or put the `update-baselines` label on, which deletes them.

Only a pull request from a branch of this repository can be labelled this way: the workflow's
token cannot push to a fork.

When the lockfile's Playwright moves, the `stories` check fails until both workflows name the
image of the new version, by its tag and the digest
`docker buildx imagetools inspect mcr.microsoft.com/playwright:v<version>-noble` prints, and every
baseline is drawn again with the label.
