# Component stories

**A component is held to what a person sees and does by its stories.** Each story is a test:
it renders in real Chromium and real WebKit, its `play` function walks it with the keyboard, axe
checks it against WCAG 2.2 AA, the words it fades are measured as drawn (#3790), and a picture
of it is compared with a baseline. CI fails on any of the five (#2901, part of the UI system
spec #2897).

Its siblings: [`ui-primitives.md`](ui-primitives.md) says what the window is built out of,
[`design-system.md`](design-system.md) what it is drawn in.

## What a story is held to

| Gate | What fails it | Where it is set |
| --- | --- | --- |
| **Renders** | The story throws, or its component does, in either browser. | `app/.storybook/vitest.project.ts` |
| **Keyboard** | Its `play` function fails: a key that does not move the focus or the value it should, Esc that does not dismiss, a focus that ends up out of sight. | the story's `play` |
| **Accessibility** | Any axe violation of a rule tagged `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa` or `wcag22aa`, with `target-size` (WCAG 2.5.8) switched on, which axe leaves off by default. | `app/.storybook/preview.tsx` |
| **Faded words** | In Chromium, a word that an `opacity` or `filter: opacity()`, on its own box or any box above it, takes under 4.5:1 (3:1 for large text) on its ground, measured as drawn. Disabled controls, words hidden from assistive technology and moving opacities are not measured (#3790). | `app/.storybook/faded.setup.ts`, `app/src/theme/fadedWords.ts` |
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
resolves now. It keeps each appearance's parts stylesheet a rule a line, so two pull requests
that change two families' parts change two lines of it (`app/.storybook/builtInThemes.ts` joins
them again). Beside it, `app/.storybook/at-creation.json` names, by theme and appearance, what a
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
  key causes with `waitFor` rather than assuming it is synchronous. Wait for the keyboard to land
  with `untilFocused(() => element, "where in the story")` from `app/src/stories.testkit.ts`,
  or anywhere in a surface (a popover's card, a submenu) with `untilFocusWithin`, never
  `waitFor(() => expect(element).toHaveFocus())` nor a `waitFor` around
  `document.activeElement` or `:focus-within`: storybook's 1 s gives up before an engine that
  draws late moves the focus, and the lint (`purlis-stories/focus-wait`,
  `app/eslint.story-rules.js`) refuses it.
- After each move, `expectFocusInSight()` from `app/src/stories.testkit.ts` checks that the focus
  is on screen and not under anything the page draws over it, such as a dialog's sticky footer
  (WCAG 2.4.11).
- The story is drawn in every cell of the theme matrix (below): never pin a theme on a story.
- The picture is of the story's frame. A story that draws outside it (a dialog, a menu, a
  popover) sets `parameters: { screenshot: "page" }`; one with nothing stable to draw sets
  `screenshot: false` and says why. Its baselines are then obsolete: delete them in the same
  change, or take the run's baselines (see Baselines). A story whose `play` ends with what it
  pictures gone, such as a dialog it answers, calls `await pictureNow()`
  (`app/src/stories.testkit.ts`) while it is drawn: the picture is taken there, and not again
  after `play`.
- A rule axe must not run on a story is turned off on that story alone, in
  `parameters.a11y.config.rules`, with the reason beside it. Never turn a rule off for every
  story.

## When a component family is done

A family's ticket (spec #2897) is done when it ships:

- a story for each variant and state that draws differently, each passing the four gates that
  run in both browsers, and the faded-words gate, which runs in Chromium;
- a `play` function on every interactive story: its keyboard path, Esc dismissal where it
  dismisses, and the focus in sight after every move;
- its baselines, drawn by its `stories` run, taken, and reviewed in the pull request;
- its parts in the theme engine's manifest, and its docs. What it adds there is `since: NEXT`
  (`super::NEXT` in the family's own file, `parts::NEXT` in a test, never imported); the pull
  request leaves `parts::VERSION` and the digest table alone, and the release step gives every
  addition of the release one version (`node tools/theme-parts-release.mjs`, ADR 0103 amended
  2026-10-25), so family pull requests never queue behind each other for the next number. A
  literal next number (`since: 13`) fails the tools' tests: write it as NEXT. A family is its
  own file and touches no list another family's pull request does (ADR 0103 amended
  2026-10-26): its parts in `crates/purlis-core/src/theme/parts_<family>.rs`, its tests in
  `parts_<family>_tests.rs` (`build.rs` finds both), its stylesheet in
  `app/src/ui/<family>/<family>.css` (the build imports it), and what is written from the
  manifest written again, never by hand: the schema
  (`PURLIS_THEME_SCHEMA_WRITE=1 cargo test -p purlis-core --lib the_published_schema`), the
  family table in `crates/purlis-core/docs/themes.md`
  (`PURLIS_THEMES_DOC_WRITE=1 cargo test -p purlis-core --lib the_docs_family_table`) and, for a
  change to a built-in theme, the stories' copy of it (below). On a conflict in one of them, take
  main's and write it again;
- every call site moved to it, with the lint rule for it at error;
- the operator's before and after look at the screens it changes.

### How to add a family

A pull request that adds a family touches its own folders and no line another family's pull
request touches (operator ruling 2026-10-26, #3386, #3387). Each shared list is found from the
files below, so nothing is added to `eslint.config.js`, `eslint.ui-rules.js`, `styles.css`, a
theme test's list or an ADR's index:

| What | Where | Found by |
| --- | --- | --- |
| its parts and their tests | `crates/purlis-core/src/theme/parts_<family>.rs`, `parts_<family>_tests.rs` | `build.rs` |
| its stylesheet | `app/src/ui/<family>/<family>.css` | `styles.css`'s `@families` line |
| its forced colours | `app/src/ui/<family>/forced-colors.css`, one `@media (forced-colors: active)` block | `styles.css`'s `@forced` line, before `theme/forced-colors.css` |
| its lint rules | `app/src/ui/<family>/lint.js`: `rules`; `off`, the files a rule is still off in; `rawButtons`, its controls still drawn as a raw `<button>` | `eslint.ui-rules.js`, every rule an error |
| its lint tests | `app/src/ui/<family>/lint.test.ts`, on `../lint.testkit` | required beside `lint.js` (`ui/lint.test.ts`) |
| what the theme tests hold it to | `app/src/ui/<family>/theme.testkit.ts`: `faint`, `quiet`, `spreadGrounds`, `classless`, `groundParts`, and `forced` (what its forced sheet draws) | `theme/families.testkit.ts` |
| the cascade tests | nothing: `cascade.testkit.ts` reads what `styles.css` imports, as built | `ui/stylesheets.ts` |
| an ADR amendment | `docs/adr/<nnnn>/<yyyy-mm-dd>-<slug>.md` | the folder; no index is kept |

The files written again from the manifest (the schema, the docs family table, the stories'
theme copy) are generated one entry per line in the families' order, so git merges two
families' additions; on a conflict, take main's and write them again. One shared list is left:
the forced-colours end-to-end probe (`app/e2e/specs/forced-colors.e2e.ts`), #3481.

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
error's message, and the script that turns the reports into annotations. The annotation is put
on the story file's line the story failed at, the first frame of its error in a story file, so it
shows on the pull request's diff; a failure with no such frame, axe's or the screenshot gate's,
is on the check alone. An error in a story
file or a suite outside any one story is an annotation of its own, and a run that fails without
naming anything says so.

## Baselines

A baseline is a PNG at
`app/src/<dir>/__screenshots__/<Component>.stories.tsx/<story id>_<theme>_<appearance>-<chromium|webkit>-linux.png`,
one for each pictured cell of the matrix.
Every baseline is drawn and compared in one image, Playwright's own Docker image for the
Playwright version the lockfile names, by the `stories` workflow's jobs
(`.github/workflows/stories.yml`): each browser's stories run in three parts, a third of the
story files each, in every cell, and one `stories` check after them names what failed in any
part and keeps the baselines they drew. It names the image by its tag and its digest,
`mcr.microsoft.com/playwright:v<version>-noble@sha256:<digest>`, so a tag pushed again never
changes a picture; the `stories` check fails unless it names the lockfile's version
(`tools/story-image.mjs`).

**A baseline is drawn only by a pull request's `stories` run, and taken only on request.** To
add or change one:

1. Push the change that adds the story or changes how it draws. Its `stories` check fails,
   naming the story, the browser and the picture that is new or differs; the pictures that
   differ are kept on the run as `story-pictures`. When a picture is all that failed (or only an
   obsolete baseline is wrong), the same run draws the stories again with `--update` in the
   pinned image and keeps every live baseline as `story-baselines`, with `drawn.json` naming the
   head and the base it tested. The check stays red: a picture counts only once it is committed.
   A story that fails any other gate draws nothing: fix it first.
2. Take the baselines, in either of two ways. Neither draws again, and both take only whole PNG
   baselines within a size and a count limit, and delete every other file in the
   `__screenshots__` directories (`tools/story-baselines.mjs take`):
   - **Your own push** (the way that runs no extra checks): in a checkout of the pull request's
     head, run `node tools/story-baselines-pull.mjs <pull request>`. It takes the newest
     `stories` run's `story-baselines` into `app/`. Review the pictures, then commit and push
     them with your next change. Your push starts every check as any push does. The tool runs
     the checked-out head's own code, so take another person's pull request's baselines only in
     a checkout you would run anyway. It reads `purlis/purlis` unless `--repo` names another,
     never `gh`'s default repository, which in a fork's clone is the fork.
   - **The `update-baselines` label**: `.github/workflows/stories-baselines.yml` waits for the
     head's `stories` run, takes its `story-baselines` with the base branch's own copy of
     `tools/story-baselines.mjs`, pushes them to the branch as one commit, takes the label off
     and comments which baselines it drew and which it deleted. It starts the `stories` check on
     the new commit itself. The other checks of a push made with the workflow's token wait for a
     maintainer to approve them in the pull request's Checks, or run on the branch's next push
     (or `gh pr update-branch`).

   Both refuse baselines drawn of another head, or of a head behind the base its run merged it
   with: update the branch, let `stories` run, and take that run's.
3. Review the pictures in the pull request's diff, which shows each one beside the one it
   replaces.

**Which `stories` run counts.** The pull request's own run, on the branch merged into its base,
is the one branch protection reads. The baselines are taken only into a head that has every
commit of the base that run merged it with, so the merge and the head are one tree.

The `stories` check also refuses a baseline the pinned image did not draw (one named for another
platform), and an obsolete one: a baseline whose story file is gone, whose story is no longer
in its file with the `test` tag, the stories the Vitest addon runs, whose cell is not one
`pictured.json` names, or whose story takes no picture (`screenshot: false`). Storybook's own
index (`storybook index`) says which stories there are; a story's parameters are in no index, so
the run itself says which take no picture (`screenshots.setup.ts` marks each in its test's
metadata, and `tools/story-failures.mjs`'s reporter writes them down). The check runs after the
stories and names the story of each obsolete baseline (`tools/story-baselines.mjs check`). When
a story is renamed or deleted, or stops taking a picture, delete its baselines in the same
change, or take the run's baselines (above), which deletes them.

Only a pull request from a branch of this repository can be labelled this way: the workflow's
token cannot push to a fork. A fork's author takes the baselines with
`tools/story-baselines-pull.mjs` instead.

When the lockfile's Playwright moves, the `stories` check fails until both workflows name the
image of the new version, by its tag and the digest
`docker buildx imagetools inspect mcr.microsoft.com/playwright:v<version>-noble` prints, and every
baseline is drawn again by that pull request's `stories` run and taken.
