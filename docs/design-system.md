# The window's design system

**A theme is a data file. Every colour in purlis comes from one, and so does every motion —
and nothing else may write either down.** `docs/ui-primitives.md` says what the window is built _out of_; this file says what
it is _drawn in_; `docs/ui-copy.md` says how it _talks_.

The rule, in one line each:

- **No colour literal outside `app/src/theme/`.** Not in CSS, not in TypeScript, not in a
  comment that becomes code. `app/src/theme/literals.test.ts` fails the build on one.
- **No arbitrary Tailwind value** — `text-[13px]`, `bg-[#fff]`, `w-[42rem]`. Same test, same
  reason: a theme cannot reach inside a bracket.
- **Every view is drawn from tokens, in both themes.** `app/src/theme/views.test.tsx` renders
  every view a tab shows, in every state it draws, in each built-in theme. It fails on what
  reaches the DOM: a colour in an inline style, an SVG paint attribute or a data URL; a `var(--x)`
  that is not a token; a token the theme in force does not set; or an arbitrary-value class. It
  catches a colour built at run time, which no source line shows. A view added to `OWN_MARKS` in
  `Views.tsx` fails that test until it has a state there.
- **Semantic names only.** A token is `surface.raised`, never `gray-800`.
- **Both built-in themes get every new token**, or the window will not start.
- **No time and no easing outside `app/src/theme/`** — no `150ms`, no `ease-out`, no
  `duration-150`, and no `prefers-reduced-motion` block. Same test; see [Motion](#motion-is-data-too).
- **Geometry is one scale, and a theme sets it within bounds.** Spacing, type sizes, weights,
  line heights, corners, control and chrome heights, focus rings and elevation are the core
  tokens in `app/src/theme/geometry.css`, which hold Purlis Classic's values. A theme folder may
  give any of them a number inside the bound the core documents, and bring its UI, mono and
  terminal fonts as woff2 files in its folder; the engine writes every property from those
  numbers and files (ADR 0097, #2326). Density and a font override are the person's, in every
  theme. See [Geometry and fonts](#geometry-and-fonts).

## Why a theme is data and not CSS

Before this layer, purlis had two colour systems. `App.css` had five custom properties and
thirty hex literals scattered through 1,330 lines; `SessionPane.tsx` built its xterm terminal
with `theme: { background: "#181818", foreground: "#d8d8d8" }` — which is `--paper` and `--ink`
written out a second time, in a second language, with nothing making them agree. The terminal's
other eighteen colours were the library's defaults and were not purlis's at all.

**The two cannot be unified in CSS**, and that is the constraint that decides the design. xterm
is handed a JavaScript object of sixteen ANSI colours plus a background, a foreground, a cursor
and a selection; a CSS custom property cannot be given to it. Unifying in JavaScript instead
would make the stylesheet a copy of the script.

So the shared thing is neither: it is a **file**. `app/src/theme/*.json` holds semantic tokens;
`theme.ts` writes CSS custom properties from a theme **and** builds xterm's object from the same
theme. One source, two consumers, and no way for them to drift. This is VS Code's model and
Zed's, and it is the only one that can colour the terminal at all.

## The vocabulary

`TOKENS` in `app/src/theme/theme.ts` is the list, with a comment on each group saying what it
means. A hundred and twenty-one colour names in twenty-three groups, and the typed tokens below
them:

| group                                  | tokens                                                                                          | what it is                                                                                        |
| -------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `surface.*`                            | `base` `sunken` `deep` `raised` `overlay` `hover`                                               | the layers of the window, deepest first                                                           |
| `control.*`                            | `base` `hover` `aimed` `count`                                                                  | things that are pressed; `aimed` is where the keyboard is, which is not where the pointer is      |
| `text.*`                               | `primary` `secondary` `muted` `quiet` `faint`                                                   | the grey tiers: four for words, and `faint` for marks and disabled words (#2902)                  |
| `border.*`                             | `subtle` `strong`                                                                               |                                                                                                   |
| `accent.*`, `focus.ring`, `tab.active` | `base` `surface`                                                                                | what purlis is drawing attention to                                                              |
| `layer.*`                              | `project` `workspace` `chat` `selected`                                                         | which of the three strips of the axis a row is, and the tab you are on                            |
| `list.*`                               | `selected` `selected-edge`                                                                      | the row a tree or a list has selected: a fill in the accent's hue and an edge down its start      |
| `tree.*`                               | `guide`                                                                                         | a tree's indent guides, one straight hairline per level                                           |
| `needs-you.*`                          | `base` `text`                                                                                   | the one signal this app exists for                                                                |
| `danger.*`                             | `base` `surface` `text` `wash`                                                                  | an answer that cannot be taken back                                                               |
| `state.*`                              | `running` `waiting` `waiting-glow` `failed` `success` `unreadable`                              | what a chat, or a check on a branch, is doing                                                     |
| `overlay.*`                            | `scrim` `shadow`                                                                                | what goes over the window when something is modal                                                 |
| `terminal.*`                           | `background` `foreground` `cursor` `cursor-accent` `selection` `find-match` `find-match-active` | the two `find-*` are a find's matches in a pane, drawn under the text (`theme.searchDecorations`) |
| `terminal.ansi.*`                      | the eight, and the eight bright                                                                 |                                                                                                   |
| `icon.*`                               | `folder` `motive` `grey` `red` `orange` `yellow` `green` `teal` `blue` `purple` `pink`          | a file's or a folder's icon in a tree; the only colours an icon theme names (FM-3)                 |
| `terminal.*` (#2327)                   | `selection-foreground` `selection-inactive` `scrollbar` `scrollbar-hover` `scrollbar-active`    | selected text (`platform`: each cell keeps its colour), a selection while the pane is unfocused, and the terminal's scrollbar at rest, under the pointer and held |
| `syntax.*`                             | `keyword` `string` `number` `constant` `comment` `function` `type` `property` `attribute` `tag` `heading` `invalid` `variable` `operator` `punctuation` | code in the light editor (`editor/look.ts`), one colour per role, each with an italic and a bold flag |
| `markdown.*`                           | `link` `code` `code-block` `quote` `heading`                                                    | a markdown render: a memory, a todo, a session record, a release's notes (`.release-notes`)       |
| `selection.*`, `caret`, `placeholder`  | `background` `text`                                                                             | the window's text selection and its words, a field's caret and its placeholder; `platform` leaves each to the platform |
| `scrollbar.*`                          | `thumb` `track`                                                                                 | the window's scrollbars; a `platform` thumb is the platform's, and the track is drawn only with a thumb |
| `chart.*`                              | `1` … `8`                                                                                       | a chart's series, a categorical palette: each point of a chart is its own series, in turn         |
| `window.*`                             | `background`                                                                                    | the native window's colour before the first frame (#2328, #2331); the page itself is `surface.base` |

**Tokens that are not colours** (#2327) sit in an appearance's `tokens` beside the colours, each
held to its own type: `syntax.<role>.italic` and `syntax.<role>.bold` are `true` or `false`;
`terminal.cursor-style` is `block`, `underline` or `bar`; `terminal.cursor-blink` and
`terminal.bold-as-bright` are `true` or `false`; `terminal.line-height` is a number from 1 to 2.
A value of the wrong type refuses the theme, as a colour that is not hex does.

**Six tokens may be left to the platform** (#2327): `selection.background`, `selection.text`,
`caret`, `placeholder`, `scrollbar.thumb` and `terminal.selection-foreground` take a colour or the
word `"platform"`, which means "draw what purlis drew before a theme could say": the platform's
selection, caret, placeholder and scrollbars, and each terminal cell's own colour when selected.
The window writes no rule for one left to the platform (`data-theme-draws`), and the contrast
check does not measure what the platform draws. A colour is drawn as given, so a transparent
thumb hides the scrollbars. Selected text is coloured only on a selection the theme colours too,
and the check says so when a theme colours one without the other. A see-through selection is
measured laid over each ground it is drawn on: the window, a field, a pane, a Notice, a dialog
and a trouble Notice, and over the danger wash laid over a dialog, where the command a profile
will run as you is selectable (#2724: a pair names up to two layers under it, laid from the
bottom up). While selected text keeps its own colour, the window's words, a markdown
render's and the danger words are held on the selection at the floor for words, the danger words
as a consent floor. Selected text a theme colours is held as a consent floor on a trouble Notice,
a dialog and the danger wash in a dialog; a theme that breaks it is drawn with the platform's selection. No palette may name
a colour `platform`. The window's rules stop at the light editor, which draws its own selection
and caret.
Purlis Classic leaves all six to the platform.
Purlis Classic gives every other new token the colour the window drew before it: a syntax role
the ANSI colour the editor borrowed for it (but light's `property` and `attribute`, `#1283ff`
rather than the bright blue's `#218bff`, so they read on the editor's current line, #2522), a
markdown link and `chart.1` the accent (and both turn with a workspace's colour, as the accent
does), markdown's code, quotes and headings the
words' colour, `window.background` the window's surface (the native window's colour, #2328), and the terminal's extras xterm's own
defaults.

**The native window follows the theme** (#2328). The operating system's own parts of the
window, the traffic lights, the menus and the dialogs, are drawn dark or light with the drawn
appearance's tag, and the window's own colour is its `window.background`, opaque. Each window is
created in both for the theme its first frame draws, so nothing of another colour shows before
it. That theme is the one the levels choose for the project and the workspace the window opens
on, resolved once as the window is created (#2602): the creation script, the native window and
the page's first frame take that one answer, and the window's later ask of what is in force
answers it again, so nothing is repainted. A project's font files are hashed once as the
window is created (#2696): the check that the remembered project is still trusted hands its
digests to the read of its theme folders, and each frame takes the faces the one before it read;
with a project at the font budget (64 files, 32 MiB), the core's part of creating the main window
measured 24 ms, against ADR 0026's 2 s start. The system's contrast is read natively at creation
where purlis can (#2582): Increase contrast on macOS, and on Linux the desktop portal's
`contrast`, which purlis hears again each time it changes, so a window created after a change
starts in the new setting. Where nothing is read, the
page picks with `prefers-contrast` among the frames it is handed for both. The page
hands both over again whenever it draws another theme
(`app/src/theme/native.ts`, `app/src-tauri/src/nativewindow.rs`). A theme that follows the
system leaves the native parts to the system. On macOS and Linux the platform keeps one
appearance for the app, so the focused window's is the one drawn. The tray and the app icon
stay as they are: they are brand, not theme.

**Forced colours win** (#2328). On a platform that forces its contrast colours (Windows'
contrast themes), the browser draws every colour in the system's palette, and what the window
says with a background or a shadow alone would be lost. `app/src/theme/forced-colors.css`, the
last layer of the window's stylesheet, says it again in system colours and with a shape: a solid
`Highlight` ring wherever Tab lands; what is selected or current in `SelectedItem` and
`SelectedItemText`, underlined (an icon tab keeps its edge); what cannot be used in `GrayText`;
each state mark's shape in `CanvasText`; a count of chats that need you in `Mark` and
`MarkText`; a dashed outline on what ends or destroys and on a ground that says something is
wrong; a switch's thumb and a radio's dot; and an outline
round what is opened over the window. It holds only system colours, never a token. The
scenario run (`app/e2e/specs/forced-colors.e2e.ts`) turns forced colours on in the real WebView
by placing the sheet's rules beside their media query, in the same layer and order, and checks
each of these on the window's own controls as computed styles in the system colours the WebView
resolves (#2583). A project's theme pick reaching the native window is checked by its own launch
(`wdio.picked.conf.ts`): the appearance the platform reads back for the window, and the
colour it holds (the window's own on macOS, its webview's on Linux), are those of the frame the
page draws, its `window.background` taken from that frame (#2760).

**Two tokens may look like one token and are not.** `needs-you.base` and `danger.base` were a
single value before this — `--stop`, `#c05c5c`, "purlis's red" — and splitting them by meaning
paid for itself on the first measurement: white on `#c05c5c` is **4.26:1**, under WCAG AA, and
the badge that failed is the count of chats waiting for the operator. `needs-you.base` is now
`#b85050` (4.88:1) and the mark on an answer that cannot be undone is untouched. A palette token
cannot make that move; that is the whole argument for semantic names in one change.

It paid again the other way (#1210). `danger.base` is the _words_ of an answer that ends
something (`.ends-it` in the answer bar a dialog's footer has since replaced, a form's `SettingActions` and a menu), and `#c05c5c` on
the button's `control.base` was **3.73:1**, under the 4.5:1 that text at the window's 14px asks
for. purlis-dark's `danger.base` is now `#c97474`, the same hue made lighter: **4.50:1** on
`control.base` (Verdigris's warm `#262623`, #1838), 4.94:1 on `danger.surface` (the same button
under the pointer, and a highlighted menu row), 4.90:1 on a menu's `surface.overlay` and 5.23:1
on the window. `danger.wash` follows
it, since a wash is its base at a low alpha. The badge kept `#b85050`; purlis-light's
`#a83232` already cleared 4.5 on each of them and did not move.

The mark moved once more (#2504): the title bar's needs-you hand is drawn on its button's
`control.hover` under the pointer, where `#b85050` measured **2.79:1**, under the 3:1 a mark is
held to. Purlis Classic dark's `needs-you.base` is now `#bd5654`, the same hue a step lighter:
**3.00:1** on `control.hover`, and the count's white on it **4.54:1**. Classic light's
`#a83232` measures 4.70:1 on its `control.hover` and did not move.

**Every theme is held to a contrast floor.** The theme engine's check (`purlis_core::theme::check`,
which runs on any theme) holds each pair that ends up as
something drawn on something: 4.5:1 for text (AA's floor for text under 24px, or 18.66px bold,
which a label, a button and a menu row all are), 3:1 for the state marks and the sixteen ANSI
colours against the terminal's own background. "Complete" is not "legible", and a light theme
made by inverting a dark one passes every other test in the directory while being unreadable.
The one exemption is `terminal.ansi.black`, held to 1.5:1 — ANSI black on a dark terminal is dim
in every theme there has ever been, because it is the colour a program picks when it means
_recede_; it still has to be visible, and purlis-dark measures 2.13:1.

**The pairs are read from the stylesheets too** (DS-6, #629). A rule in `App.css` or
`styles.css` that sets one token as its text and another as its background draws one on the
other, and `contrast.test.ts` fails, naming the rule, until that pair is in the engine's list
(`PAIRS`) at the floor its use asks for. A background that is a wash (a token with alpha) is measured laid over what it
sits on. The check reads one rule at a time, so a colour taken from a parent's background, or a
`:hover` rule that sets only a background, is still listed by hand. Every built-in theme is
checked, as it ships and tinted by every palette colour.

**The `layer.*` tokens are a shade that means _depth_, and nothing louder.** purlis#171
said which of the three strips you were looking at with an indent; purlis#193 took the
indent away on the operator's reading of it. Coloured rules under each strip replaced it for one
review and were turned down on sight — _"this is not looks professional, it should be
minimalistic, and i prefer to change little bit backgrounds of tabs and little lighter for
selected tab, borders are not feeling well."_ So each strip is one small step of neutral grey,
outermost deepest, and `layer.selected` is a step lighter than the lightest of them; that
background is the whole of the selection signal. **Both themes step the same way** — deeper
outside, lighter in, lightest where you are — and the contrast check holds a strip's muted tab
text to 4.5:1 on each shade and the selected tab's primary text on `layer.selected`. Their own
group rather than `surface.*`, so a theme author can move the axis without moving every other
surface in the window.

`layer.divider` is the one line the axis allows: a very light hairline between two tabs of a
strip, never under or around one — the operator's _"very very light visible border - just for
little bit highlight separation"_. Its own token because `border.subtle` sat too close to the
layer shades to be seen, and a theme should be able to lift the tabs apart without lifting every
other subtle rule in the window.

**One tree style, and a selected row that looks selected** (#1672). Every tree in the window —
the Chats tree, the explorer, the Changes view and a file tab's files — is drawn by the shared
tree row (`.ui-tree` and `.ui-row.ui-tree-row` in `primitives/collections.css`, DS-3, every tree
moved onto it by DS-11 and DS-12), and not by a shared component (ADR 0037): each tree keeps its
own markup and keys, and the class draws its levels, its guides, its selected row and where the
keyboard is. The `.tree` class in `App.css` that drew them before is gone.

- **A level is one step in and one straight guide**, VS Code's: a hairline in `tree.guide` down
  the whole level, under the row it hangs from. Each row says its level (`aria-level`, or
  `data-level` on what holds it), nested or flat, and draws one guide per level above it as a
  background of `--ui-tree-step` columns; what a row holds under it (a sentence, a branch's
  marks) carries the children's level on the element that holds it. No elbows: an elbow has to know where its row's first line
  is, and every time a row's metrics moved the elbows pointed above the name. A straight line
  knows nothing about the rows beside it and masks nothing, so it is right on whichever surface
  a region is moved onto (ADR 0038).
- **Selected is `list.selected` and an edge of `list.selected-edge`**, on a row that is
  `aria-current` (a tree that marks the current item: the chat in front, the spot the next chat
  starts in) or `aria-selected` (one that selects: the file a file tab shows). It is not
  `surface.hover`, which is the pointer and which a selected chat used to be drawn in, so the
  chat in front and a hovered one looked the same; and it is not `layer.selected`, which belongs
  to the three strips of the axis. The edge is a shape, so selected is never told by colour
  alone; it is an inset shadow and not a border, so selecting a row moves nothing in it. Both
  turn with a workspace's colour, as the accent does (`TINTED_WINDOW`).
- **The keyboard's place is a ring and only a ring**: the window's own `focus.ring` outline
  round the row, with no fill, so a focused row is never read as selected and the selected row
  the keyboard is on shows both.

The contrast check holds everything a row draws on `list.selected` — primary, secondary and
muted text at 4.5:1, the state marks, the needs-you hand, the edge and the focus ring at 3:1 — and the guide on
each surface a tree is drawn on, and holds `list.selected` apart from `surface.hover` and
`layer.selected` in every theme and tint. The ceiling that sets is the needs-you hand: a fill
light enough to read from across the room would take `needs-you.base` under 3:1 on it in
purlis-dark (3.08:1 on Verdigris's `#112b25`, the closest pair to its floor), so the dark fill is
a deep accent teal and the edge does the shouting. The
gallery (`gallery.test.tsx`) draws a tree of each shape in both themes, and
`ChatsSection.window.test.tsx` holds the chat in front to the selected look, a hovered row to
the hover's, and the keyboard's ring to neither, against the whole window
(`cascade.testkit.ts` picks the winning declaration among `App.css`'s top-level rules by
specificity and order, with `:hover` and `:focus-visible` as asked, since jsdom does neither;
what it answers is the declared value, and a narrowing `@container` query is not in force). The
Chats tree has since moved to the shared tree row (`.ui-tree-row`, DS-3 and DS-11 in
`ui-primitives.md`), which draws the same guides, selection and ring.

**A workspace's colour is a hue shift of the theme in force, not a colour of its own**
(purlis#281, ADR 0048). A workspace names one of eight colours (`red` to `pink`, the names the
core reads) or a `#rrggbb` whose hue is taken. **Which tokens it turns, and the hue each of the
eight gives, are the theme's tint recipe** (#2329, the theme's `tint`, below), which the engine
resolves and hands the window with the theme (`purlis_core::theme::Tint`, `theme.tint`); the
window keeps no list of its own (#2393). Purlis Classic's recipe, the one a theme starts from:

- on the whole window, from the workspace in front: `accent.base`, `accent.surface`,
  `focus.ring`, `tab.active`, `list.selected`, `list.selected-edge` (`tint.window`, drawn by
  `drawTint`);
- on that workspace's own tab and on the chat strip, which holds its chats, the same and
  `layer.workspace`, `layer.chat`, `layer.selected` (`tint.tabs`), set on the element as
  custom properties by `tabTint`. Each workspace tab carries its own tint whether or not it
  is in front: a `.workspace-mark` dot in its accent and its own shade. (The title bar's
  breadcrumb carried the same dot until ADR 0054 took the breadcrumb away.)
- the hues `red` 25°, `orange` 55°, `yellow` 95°, `green` 145°, `teal` 190°, `blue` 255°,
  `purple` 305°, `pink` 350° (OKLCH).

The strip shades (`layer.workspace`, `layer.chat`, `layer.selected`, `layer.divider`) are only ever
tinted on a workspace's own tab and chat strip, never on the window. `app/src/theme/tint.ts` does
the arithmetic in OKLCH, the same as the engine's (`purlis_core::theme::tint`, held to one another
by `theme/tint-cases.json`), and then keeps each token's **relative luminance**: each channel is
rounded to the byte that keeps the luminance nearest the original on its own side of WCAG's middle
(where black and white contrast with a colour alike), so a dark shade is never rounded lighter and a
light one never darker, and a tinted shade keeps every contrast floor the theme's own kept against a
colour across the middle, at every hue (#2526). The contrast check runs every hue of the theme's
recipe, and holds two pairs the tint adds — the mark (`accent.base`) and a tab's primary text on
`layer.workspace`. **Every tint holds the consent floors that keep colours apart itself, at every
hue, custom colours included** (`tint::APART`, OKLab 0.1 each): the accent and the accent-hued
tokens drawn beside danger (`focus.ring`, `list.selected-edge`, `accent.surface`, `tab.active`, and
the primary's `accent.fill` and `accent.fill-hover` and its edge `accent.fill-edge`,
`tint::ACCENT_KIN`) stay apart from the danger colour, and Allow and Keep blocked stay apart
(`accent.base` from `border.control`, as the tint draws it); every rule holds whichever side a theme
tints. At a hue that would bring a pair nearer, the tint keeps the hue and gives less colour, in
steps of 0.005 chroma, so a red workspace stays red, and a grey accent keeps as much of its small
tint as holds it apart; a colour no amount holds apart is drawn untinted. Purlis Classic's light
appearance gives less colour at `red`, `orange` and `pink`. The consent floors are also measured
under every tint of the theme's own recipe, danger apart from the accent and its kin included, and a
pair a tint can move from one hue to another (words on a wash, faded words, a colour written
see-through, or a shade read against one on its own side of the middle) at every whole degree of a
workspace's own colour. A theme that clears the floors as written and would fall under one only in a
tinted workspace keeps its values, and the tokens of that pair are drawn untinted. A neutral grey is
given a small fixed chroma, felt rather than noticed, as the strips are meant to be; white stays
white. No colour is written for this anywhere but `app/src/theme/`: a component asks `tabTint` and
sets what it answers.

**A persona's mark is tinted the same way, on two tokens only** (#1449), at the hues of the
theme in force, whether or not its recipe tints workspaces: a persona's colour says who it is. A persona is drawn
wherever it appears as one `PersonaMark` (`app/src/PersonaMark.tsx`): its custom `icon.png`,
else its built-in icon, else its initials, on a ground of its colour. The colour is the
workspace's vocabulary, one of the same eight hues or a `#rrggbb`, and a persona that names none
is given one of the eight by its name (`colourOfName`), so it is the same on every machine.
`tintVariables` turns `accent.base` and `accent.surface` to it (`MARK_TOKENS`) on the mark's own
element and nowhere else: the mark is a small ground with a glyph on it, and the contrast pair
it needs is the one the workspace tint already holds. No colour is written in the component.

**The chat states and the CI states share a group on purpose.** `.ci-pending` is
`var(--state-waiting)` because amber means "not finished" in both, and a theme author who wants
to change that changes one value rather than hunting for the second one.

## The brand: Verdigris

purlis ships two themes: **Purlis**, the default (below, #3034), and **Purlis Classic**, the
alternative, one theme folder with a dark and a light appearance
(`crates/purlis-core/src/theme/built-in/purlis-classic/theme.json`), picked as
`purlis-classic` with an appearance. Both appearances are drawn in **Verdigris** (#1838, the operator's D1 and D4 on #1832): a slightly desaturated teal
accent on warm-grey neutrals. It is calm and technical, easy on the eyes in a long session, and
not the blue most developer tools use.

| | dark | light |
|---|---|---|
| `surface.base` / `surface.raised` / `surface.hover` | `#191917` / `#1b1b19` / `#242421` | `#fcfbf8` / `#f0eee8` / `#ebe9e2` |
| `text.primary` / `text.muted` | `#dddcd6` / `#9e9d94` | `#1f1e1a` / `#605d54` |
| `accent.base` (= `focus.ring`, `list.selected-edge`) | `#52c4a6` | `#086a56` |
| `accent.surface` / `list.selected` | `#17302a` / `#112b25` | `#dcf1ea` / `#d3ede4` |

- **The neutrals are warm graphite and paper.** The dark base is lifted off black and the light
  base is off white, which cuts glare without costing contrast: text is 12.81:1 on the dark base
  and 16.12:1 on the light one, muted text 6.46:1 and 6.36:1, the accent 8.22:1 and 6.33:1.
- **The status colours did not move.** Running blue, waiting amber, failed red, success green,
  needs-you and danger keep their values, so a state means what it meant. The accent is a teal,
  which sits near success's green; a state is always said with its mark and its words, never by
  colour alone, which is what keeps the two apart.
- **The terminal follows the window.** Its background, foreground and cursor are the theme's
  base and primary text, and its selection is the accent at the same alpha as before. On the
  off-white light base, `terminal.ansi.bright-white` is `#7d858f`, darker than before, so it
  still clears the 3:1 an ANSI colour is held to.
- **Every pair holds.** The theme engine's check (`purlis_core::theme::check`) passes all its
  pairs, the ANSI floors, the selected row's distinctness and every tint in both themes. The closest pair is `needs-you.base` on the
  dark `list.selected`, at 3.08:1 against a floor of 3.

**The ids.** A theme is picked by its id: in a project's `[theme] use`, a workspace's
`settings.theme.use`, and the window's own choice. The ids the two themes had before the rename
are still read, as the renamed theme, until the rename's compatibility window closes (ADR 0091).
They are spelled in two places: the core's name module (`purlis_core::names::THEME_DARK`,
`THEME_LIGHT`), which `extension::project::theme::Pick` and the theme engine's `built_in` read
a pick through, and the window's alias table (`THEME_ALIASES` in `theme.ts`), which labels one
in Settings.
`themeIds.test.ts` holds the two tables to each other and fails on an old id spelled anywhere
else in the window; `no_old_name_is_spelled_outside_the_name_module` does the same for the Rust.
A file that still names an old id is shown in Settings under the renamed theme's label. Nothing
rewrites the file; the next pick made in Settings writes the new id. Now that Purlis is the
default (#3034), `purlis-dark` and `purlis-light`, and their old ids, name Purlis's dark and
light (`theme::shipped_pick`): they were written as the default's dark or light, so they draw the
default's. Settings no longer offers them, and labels a file that holds one. The old `system` id
follows the system with the person's own theme, else Purlis. Only `purlis-classic` names
Classic.

### Purlis, the default theme

**Purlis** (#2336) is the second built-in theme, built from the designer's foundation
(`docs/design/foundation/README.md`): a lavender accent on cool dark neutrals, in IBM Plex Sans
for the interface and JetBrains Mono for code, paths and the terminal, with Lucide's icons. Its
folder is `crates/purlis-core/src/theme/built-in/purlis/`, compiled into the core and read by
the same reader as any theme folder, its fonts and icon set included (`theme::purlis`). A pick
names it `purlis`; no theme folder takes that id, and a theme may `extend` it.

**It is the default** (#3034, the operator's ruling of 2026-10-22; folds in #2338): where no level
chose a theme, the window draws Purlis's dark (`theme::DEFAULT`, `theme::default_theme`), or its
high-contrast dark when the system asks for more contrast, and a pick that cannot be drawn draws
it too. The window is handed it as it is created (`default_theme`, and by `purlis-dark` beside
Classic's dark by `purlis-classic`, `windowprefs.rs`); Purlis's light is not handed, since its
parts are measured across every tint before they are drawn, and a pick of `purlis-light` asks
the core. Its faces wait for the first frame to be on screen (`holdFacesForTheFirstFrame`,
ADR 0026's 2 s cold start). The gallery and `purlis theme list` show it first, then Purlis Classic, and
a new theme draft extends it. No stored value is rewritten; how one is read is the table under
**The ids** above: `purlis-classic` draws Classic, and every other built-in id draws Purlis. A
token a theme does not name still falls back to Classic's of its appearance, so a theme written
before the switch draws as it did. A workspace's colour dims only the tokens a tint rule holds
apart from danger (`tint::chroma_for`); its strip keeps the colour's full hue in Purlis too.
No built-in theme ships as a preview now; each built-in theme's entry in `purlis theme list
--json` keeps its `"preview"` key, false for both. The component stories run in every cell of
the theme matrix, and Storybook starts on the default's cell, which
`app/.storybook/at-creation.json` names and a test of the app's core holds to the engine
(`docs/component-stories.md`).

It has three appearances, `dark`, `light` and `dark-high-contrast` (dark, high contrast), each
naming every token itself, so nothing in it is Purlis Classic's by falling back. Every
combination clears the theme check with no warning and holds no consent floor, and a test keeps
it so (`purlis_tests.rs`). **Light and high contrast ship as drawn** (the operator's ruling of
2026-10-24, after seeing them beside dark and Classic, #2924). Each can be chosen in Settings, and
Purlis chosen to follow the system draws light on a light system and high contrast where the
system asks for more. With nothing chosen, Purlis draws its dark (its high contrast where more is
asked for), whatever the system's appearance. Where its values come from:

- **Dark is the foundation's.** Each colour the record proposes for a token is that token's.
  Where the record lacks a value, one rule derives it from the record's palette:
  - a wash the record gives with alpha is laid over the surface the record draws it on, so the
    selected row is `#2a2936`, one step under `#2b2a37`, the colour the record read off the
    frame, so the quiet and faint tiers clear their floors on it (#2902);
  - a colour the record has no role for (running, failed, unreadable, danger, needs-you, the
    file icons' hues, most of the terminal's sixteen) is Purlis Classic's hue for that role at
    the lightness and chroma of the record's two state colours, `#e5c08a` and `#8fc79a`
    (OKLCH L 0.803, C 0.084); a bright ANSI colour is one step lighter, the step between the
    record's two greens;
  - the needs-you count is the window's base on its chip, which is light;
  - the code colours the record names are the syntax roles they draw (keys, strings, literals,
    punctuation), and the other roles take Classic's ANSI colour for them.
  **The grey tiers** (#2902, ruling #2803) are primary `#e6e4ec`, secondary `#c9c7d2`, muted
  `#a09eab` and quiet `#91909d` for words, and faint `#74727f` for what is not words. Quiet is
  the record's `#8a8896` lifted until it clears 4.5:1 on every ground, the row under the pointer
  included; faint merges the record's `#6f6d7a` and `#55535f` and clears 3:1. Quiet is
  informational text, and none of it is exempt: line numbers, placeholders (`placeholder` is
  quiet), counts, paths and group labels. Faint is a tab's ×, a separator, a pin, a chevron and
  disabled words; informational text never takes it.
- **Geometry is the foundation's where a core token draws the same thing**, on the record's
  corner scale, 7, 8, 10, 12 and full (R157, R158): a small corner and a menu row's 7
  (`radius.sm`, `radius.md`), an md control's 8 (`radius.control`: buttons, fields, selects, a
  list's rows, title-bar chips, a tab's top), menus' and dialogs' 10, panels' and the Welcome
  doors' 12; a control 28 high (#3113, the ruling on #2793; the record's buttons were 26) and a
  chrome row 24; controls' and a dialog's words at 13 (`font-size.base`, the record's 12–13) and
  headings at 15 and 18 (`font-size.lg`, `font-size.xl`, R160); body text at a line height of
  1.55, and no shadow under a raised surface. Every corner a stylesheet draws in Purlis is on
  the scale, and `app/src/ui/corners.test.ts` holds that.
- **The Figma pass** (#3177, the operator's ruling of 2026-10-24) took the audit's fifteen
  deviations out at once. **No hairlines**: a plain control's edge (`border.control`) is
  transparent, and the dialog, the popover and the tooltip draw none (their parts); Allow keeps
  its accent edge and its lock. **The primary is filled**: `accent.fill` `#6d58bb` with white
  words (`accent.fill-text`), 5.6:1, and `#745fc3` under the pointer, 5.0:1, in dark, light and
  high contrast alike, with no edge (`accent.fill-edge`, transparent: a token, since a part on
  the button would have its consent floors measured at every hue at every launch); Allow, on the control's ground behind the
  accent's edge, stands farther from it than before. **A chosen activity button** is the
  record's .14 wash (`accent.wash`) with its icon in the lighter lavender (`accent.text`,
  `#c3baf5`), and the update pill's words take `accent.text` too. Section labels are tracked at
  .05em in every theme. Purlis Classic draws as it did: each new token is the value of the one
  that drew it before, and its parts keep its 9px rail buttons, its 5px title-bar chips and its
  update pill's primary words.
- **Light is derived from dark by one rule**: each colour's OKLCH lightness is reflected about
  the record's deepest colour (`#121215`, L 0.184), so L becomes 1 − (L − 0.184), keeping its
  hue, chroma and alpha. A reflected colour that misses a floor of the check steps its
  lightness away from its ground by 0.01 until it clears: the comment and punctuation grey, on
  the editor's current line (`#7c7a84`), and the quiet and faint tiers on the row under the
  pointer (`#61606c`, the placeholder too, and `#7c7a87`). **A wash is not reflected** (#2924):
  a hue reflected dark and laid thin on a light ground reads as grey, so `wash.warning`,
  `danger.wash`, `diff.deleted` and `diff.inserted` are each their hue's purest colour, where
  sRGB holds the most chroma for it, at dark's alpha: amber `#fdb0051f`, red `#fd04441f`, green
  `#01fd781f`. Laid on any light ground, each keeps its hue within 40 degrees and at least 0.02 of
  chroma, and a test holds that.
- **High-contrast dark** moves each text step up one (primary to white, quiet to the muted
  `#a09eab`, faint to quiet's `#91909d`), each edge up to a text grey, and the accent to the
  record's lighter lavender `#c3baf5`. Its strong border and tree guides are the palette's
  quiet, so they moved with it from `#8a8896` to `#91909d` (#2902): a lighter edge on dark.

**The fonts are bundled as their projects publish them**, unmodified: IBM Plex Sans 3.005
Regular, Medium and SemiBold, each upright and italic (IBM's complete woff2, which covers Latin,
Greek and Cyrillic), and JetBrains Mono 2.304 Regular, Italic and Bold. Both are under the SIL
Open Font License 1.1, whose text is in the folder's `LICENSE` beside Lucide's ISC licence. Only
the weights and styles the theme draws are shipped, 689 KB in all. They are not cut down
further: IBM Plex reserves its name, so a cut-down copy could not be called Plex, and the
terminal needs JetBrains Mono's box-drawing and arrows. The window is served each face from the
bytes compiled in, by its digest, as it is any theme folder's.

**Each style Purlis draws has a face of its own** (#2709), so the window never slants or
thickens a face to make one. `theme::styles_drawn` names every style a resolved theme draws:
the interface at each of its three weights, upright and italic for emphasis; code upright, and
each syntax role at its italic and bold (comments italic, tags and headings bold, at
`theme::BOLD`, 700); the terminal upright, bold and italic. `theme::without_a_face` names those
its fonts have no face for, and a test holds it empty for every appearance of Purlis. The theme
check warns once for each of them, naming the slot and the style (#2736). A font with no face is
the platform's own, as Purlis Classic's are, and brings its own styles.

## What a theme looks like

**A theme is a folder** (format 2, spec #2313): a `theme.json`, and beside it, optionally, its
`fonts/`, `icons/`, a preview image and a `LICENSE`. A lone `theme.json` is a whole theme.

```json
{
  "$schema": "https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json",
  "name": "Dusk",
  "extends": "purlis-classic",
  "palette": { "lavender": "#b4a7f5" },
  "appearances": [
    { "id": "dark", "tokens": { "accent.base": "lavender", "focus.ring": "lavender" } },
    { "id": "dim", "appearance": "dark", "high-contrast": true, "tokens": { "text.primary": "#ffffff" } }
  ]
}
```

- **`$schema`** names the published schema, which the theme engine generates from its own
  vocabulary (`purlis_core::theme::schema`, held to `docs/schema/theme.v2.schema.json` by a
  test): an editor completes every token name and flags a value outside the grammar. See
  [Which schema a theme names](#which-schema-a-theme-names).
- **`extends`** names the theme this one starts from: `purlis-classic` (or an id a pick names
  one of its appearances by), or another theme folder by its name. The theme holds only what
  differs. A parent that is not there, a parent purlis cannot draw, and themes that extend one
  another in a circle each refuse the theme with a sentence that names them.
- **`palette`** is named colours, hex only. A token's value is hex or a palette name: a closed
  grammar, never CSS. A name is the theme's own; a child cannot name its parent's.
- **`appearances`** each have an `id`, a tag (`appearance`: `dark` or `light`) and optionally
  `high-contrast`, with their `tokens` and `motion`. One whose id its parent has layers over
  that one and keeps its tag; a new one starts from Purlis Classic's appearance of its tag. The
  first appearance is what a theme draws when nothing chose another; following the system works
  with any theme that has a dark and a light one, and following a theme of one appearance stays
  in it, which Settings › Theme says on its Follow the system (#2331). The old `system` id names
  the person's own `theme.json`, else Purlis Classic, followed as any theme is (#2615). **When the system asks for more contrast** (`prefers-contrast: more`: Increase
  contrast on macOS, a high-contrast theme elsewhere), whatever draws an appearance of a tag
  draws the theme's appearance of that tag tagged `high-contrast` instead, when it has one, and
  the window draws again when the setting changes (#2328). A built-in pick names its appearance.
  A theme has at most 64.
- **`icons`** names the theme's icon set (#2337): the path of a JSON file inside the theme's
  folder (`"icons": "icons/icons.json"`), read as `theme.json` is: no link on the way, a plain
  file, under the same cap. The file draws each view's tab on the rails, by the window's own
  view ids, as path data:
  `{ "views": { "chats": { "paths": ["M22 17a2 2 0 0 1-2 2H6.8…"] } } }`. Each path is the icon
  theme's path grammar on a 24 grid, at most 16 to an icon, and the window strokes it 2 wide with
  round caps and joins in the rail's own colour for the tab's state; none of that is the set's to
  say. A view the set does not draw is its parent's, and a view no theme draws is the window's
  own mark, so Purlis Classic, which names no set, keeps today's icons. A view name purlis does
  not have is said and left out. Purlis's own set is Lucide's icons, converted
  (`app/src/theme/railIcons.vendor.ts`, with Lucide's licence in the folder).
  The set may also draw the **harness glyphs** (#2922), under `glyphs`, by name: `spark` (Claude
  Code), `braces` (opencode) and `terminal` (Codex), held to the same rules
  (`{ "glyphs": { "spark": { "paths": ["M12 2.7v18.6"] } } }`). A set names views, glyphs or
  both. They are what a harness and a profile that names a glyph are drawn with
  (`app/src/ui/harness-icon/HarnessIcon.tsx`); a glyph no theme draws is the window's own
  (`app/src/theme/harnessGlyphs.ts`), and Purlis's set ships the same three, drawn by purlis. A profile's icon, a glyph or the
  person's own SVG (drawn in the text's colour alone), is drawn only beside the profile's name,
  never on a consent or ask surface.
- **`options`** are the theme's toggles and choices (#2325): each has an `id`, a `kind`
  (`toggle` or `choice`), a `label`, an optional `description` and a `default`. A toggle's `on`
  is a layer drawn when it is on; a choice has at least two named `values`, each a layer. A
  layer is `tokens` for every appearance and optionally `appearances`, token values for one
  appearance by its id, drawn over the first; a layer holds what an appearance's `tokens` may,
  geometry within its bounds included. There are no free-number options. An option
  nothing chose draws its default. A theme has its parent's options, and one of its own with a
  parent's id takes that one's place. The theme check covers every appearance × option
  combination, so a theme has at most 64, and one past that is refused with its count. The
  theme engine takes the option values as an input (`Theme::with`), by the same precedence as
  the appearance, and Settings › Theme sets the chosen theme's options at each level with where
  each value comes from (#2527).
- **`tint`** says how a workspace's colour tints the theme (#2329), the same in each of its
  appearances: `"tint": { "on": true, "tokens": ["accent.base", "surface.raised"], "hues":
  { "red": "#c0392b" } }`. `on: false` and a workspace's colour tints nothing in it. `tokens`
  replaces the parent's list, from a closed list: the surfaces, controls, words, rules, the
  accent and its kin, the selected row, the tree's guides and the workspace's strip shades
  (`vocabulary::TINTABLE`). The consent and state colours (`needs-you.*`, `danger.*`,
  `state.*`) are purlis's to hold, the diff washes (`diff.*`) and the icon colours (`icon.*`)
  carry their meaning in their hue, the terminal (`terminal.*`) is handed its colours once, the
  project strip (`layer.project`) is not a workspace's, and the scrim and shadow (`overlay.*`)
  lie over every workspace at once: naming one refuses the theme. `hues` gives any of the eight
  colours this theme's hue, as a hex colour or a palette name whose OKLCH hue is taken; a grey has none and refuses the theme. A
  workspace's own `#rrggbb` is always its own hue. What a theme does not say is its parent's,
  and Purlis Classic's recipe is the start.
- **A format 2 theme that breaks its grammar anywhere is refused whole** (ADR 0097 §2): purlis
  draws Purlis and says what is wrong.
- **Geometry and fonts** (#2326): an appearance's `tokens` may give a geometry token a number
  inside its bound (`"radius.md": 6`), a `compact` object tunes what compact density draws for
  every appearance, and `fonts` names the UI, mono and terminal fonts from the folder's
  `fonts/`. [Geometry and fonts](#geometry-and-fonts) has the bounds.

**Format 1**, a single file with one appearance, keeps loading as a theme with that one
appearance:

```json
{
  "name": "purlis-dark",
  "appearance": "dark",
  "tokens": {
    "surface.base": "#181818",
    "terminal.ansi.red": "#ff7b72"
  }
}
```

- **`appearance` is `dark` or `light`.** It decides two things: which of Purlis Classic's
  appearances fills in the
  tokens this file does not name, and what `color-scheme` the document gets, which is what
  makes the platform's own scrollbars and form controls match.
- **A value is hex and only hex** — `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`. This is a security
  boundary, not a style preference: a value is written straight into a CSS declaration, so
  `red; } body { display: none` in a theme file would be a stylesheet somebody else wrote. The
  grammar removes the question rather than answering it, and every value it allows is one xterm
  also accepts. Converting an `rgb(0 0 0 / 55%)` to `#0000008c` costs an author one lookup.
- **A partial theme is normal.** Name the tokens you are changing; the rest come from
  Purlis Classic's appearance of the same tag.
- **A theme that breaks its grammar is refused whole, in either format and from any source**
  (ADR 0097 §2): a value outside its type or bound, a key format 2 does not have, geometry, or
  a file that is not a theme. purlis draws Purlis in its place and a Notice names each value.
  A token the theme does not name is not an error: it is Purlis Classic's. A token or motion
  name purlis does not have, and a key format 1 does not have (`author`, `description`), is said
  and left out, and the rest is drawn: a teammate's purlis may know a token this one does not
  yet, and format 1 files carry keys of their own.
- **Nothing in a theme file can stop the window.** The theme engine's `load` always answers with
  a complete theme, because ADR 0026 holds cold start at 2 s and a window that will not come up
  because a colour was spelled wrong is worse than every possible wrong colour.

### Which schema a theme names

**Name the schema of the purlis the theme is for**, at its release:

```
https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json
```

`<version>` is the one `purlis --version` prints, and `purlis theme list --json` gives the whole
address as `theme_schema`. Purlis Classic's folder names its own release's, and so does every
theme purlis writes.

- **Why a release, not `main`.** `main`'s schema has every token the next purlis will know, so
  an editor pointed at it completes tokens an installed purlis leaves out. The address at a
  release tag is that release's schema, and stays there for as long as the tag does: a theme
  written for 0.4.2 still opens with 0.4.2's completions after 0.5 ships.
- **Why the format is in the file's name.** Within one format the schema only grows: a release
  adds tokens, keys and accepted values, and never takes one away or narrows one, so a theme
  valid against an earlier release's schema is valid against every later one of that format.
  A test (`theme::published`) refuses a schema that removes or narrows anything against the one
  committed for its format, and CI's `rust` job holds it to the one the last release's tag
  holds too (`tools/theme-schema-since-release.sh`); such a change is a new format, with a file
  of its own. Under `not`, `if` and a `oneOf` whose branches no constant tells apart, a wider
  schema can accept less, so whatever a reference there reaches stays as it was.
  The check reads `patternProperties` with Rust's `regex` semantics, not ECMA-262's.
- **Why GitHub's raw file and not the docs site.** The tag is what a release is, and a raw file
  at a tag cannot change after the release; the docs site is one version, the newest, and is not
  published yet. purlis never fetches the address (ADR 0083's amendments of 2026-10-17): the
  theme engine reads a theme against its own vocabulary, and only an editor opening `theme.json`
  reads the schema.
- **A dev build** names the release it becomes, which is not tagged until it ships. Until then,
  point `$schema` at `main`'s file (`…/purlis/purlis/main/docs/schema/theme.v2.schema.json`)
  and expect it to offer tokens the dev build may not have yet; purlis says such a token and
  leaves it out.

### The theme engine

**One theme engine, in the Rust core, owns everything about a theme** (`purlis_core::theme`,
spec #2313): reading a theme from every source there is (the built-ins compiled into it, the
person's `theme.json`, an approved extension's theme, the pick a project or workspace makes),
holding every value to its grammar, filling what is missing from the built-in of the same
appearance and saying so, deciding which theme the window draws, and checking whether its words
can be read. Its interface:

- `load(document)` reads one theme document, format 1 or 2, and always answers with a complete
  theme (every appearance) and what it had to put right or why it refused it; a `Library` reads
  one that extends a theme folder, and `load_folder` reads a folder;
- `resolve(inputs)` answers which theme and which of its appearances the window draws, from the
  project's pick, the person's own theme, the extension themes in force and the operating
  system's appearance;
- `schema()` is the published JSON Schema of format 2;
- `check(theme)` holds any appearance to every contrast pair (`PAIRS`), the ANSI floors and the
  selected row's distinctness, as it is and tinted by every hue of its own recipe, and reports
  each finding with its ratio.

**A path is said relative to its level** (#2763). Every sentence the engine says about a file
or a folder of a level's theme folders names it by the level and its path there: "the project's
themes/dusk/theme.json", "your purlis config folder's themes/dusk", "the workspace alpha's
themes/dusk", "the chat's theme draft dusk/theme.json". Each complaint carries the same place as
data (`Complaint::at`, a `Placed`: the level and the path). So the window, the command line and
a chat's theme specimen show what the engine said as it said it, and none of them looks for a
path in a sentence.

**The consent floors hold whenever a theme is drawn** (ADR 0097 §6). Every answer `resolve`
gives, the theme the window is handed as it is created, and the theme specimen, is drawn by
`draw`, which holds five floors for any theme from any source. `draw` takes an appearance with
its theme options' values already layered on it, so the floors hold in every combination of
appearance and option values, and no option value can bring a consent surface below its floor:

- **an ask contrasts with its surface**: `text.primary` on `surface.overlay` (solid, and at the
  0.8 opacity a dialog's `.honest` words are drawn at), on `control.base` and `control.hover`
  (an answer's button, Allow's and Keep blocked's alike, at rest and under the pointer) and on
  `danger.wash` over the window and over a dialog, and `needs-you.text` on `needs-you.base`;
- **Allow is distinguishable from Keep blocked**: on every ask of the two they carry equal
  weight. Keep blocked is the plain `Button`, edged in `border.control` (none in Purlis); each
  Allow is `variant="grants"`, on the same `control.base` ground, edged in `accent.base` and led
  by an open lock, a cue that is not colour. The two edges are at least 0.1 apart in OKLab as the theme
  writes them, and `accent.base` clears 3:1 on `control.base` and on `control.hover`, so the edge
  that tells them apart is seen. `variant="primary"` stays the answer purlis suggests and is
  never a grant's;
- **the danger colours contrast with what they are drawn on**: every pair of `PAIRS` whose front
  is a `danger.*` token;
- **danger is distinguishable from primary**: `danger.base` and `accent.base` at least 0.1 apart
  in OKLab, five times the least difference a person can see, as the theme writes them;
- **the state marks meet the mark floor**: every pair of `PAIRS` whose front is a `state.*`
  token or `needs-you.base`, the title bar's needs-you hand on its hovered `control.hover`
  included.

Each contrast floor is the pair's own floor in `PAIRS`, held as the theme is and tinted by every
workspace palette colour. When a theme breaks one, purlis draws Purlis Classic's values of the same
appearance for the `danger.*`, `needs-you.*` and `state.*` tokens of each pair that fails, and
for the pair's other token when Classic's colour still fails on it. A floor between tokens none
of which is a consent token (Allow apart from Keep blocked, an answer's words on its button) is
held by drawing Classic's values for each of them. The rest of the theme draws
as written, and one Notice in the Inbox names the theme and the floors it breaks.

**The window only applies.** It is handed a resolved theme, every token's final value and the
motion, and writes the custom properties and xterm's object from it (`theme.ts`). The first
frame's theme is resolved before the window exists and handed to it with the window, so nothing
is read or asked on the way to the first paint; a project's pick and an extension's theme are
asked for afterwards (`theme_in_force`). The window still names the token, motion and geometry
lists it writes, and `core.test.ts` holds each to the engine's; the tint recipe comes with each
resolved theme; the stylesheets' half of the
contrast suite stays in `contrast.test.ts`, which reads the engine's `PAIRS` and fails for a pair
a stylesheet draws that the list does not hold.

**The command line asks the same engine** (#2321, #2518). `purlis theme list` shows every
theme available on the machine and in the plane, its appearances, its level and where it comes
from (Purlis Classic built in, the person's `theme.json` as `mine`, a level's theme folder as
`machine:<folder>`, `project:<folder>` or `workspace:<ws>:<folder>`, an approved extension's as
`<extension>/<theme>`). The levels' folders are read through `Folders`, the same read the window
makes, so the command line has no reader of its own for them. `purlis theme check <theme>` (an
id, or the path of a theme file or folder) prints the check: why the theme is refused, what was
left out of it, each appearance's contrast findings with the pair and ratio as warnings, the
consent floors the window holds in each combination with the tokens it draws as purlis's own,
and the hard floors, as text or as `--json` for a chat. It
exits 0 when the theme is drawn, 1 when it is refused, 2 when there is no such theme.
**A chat's theme draft** (#2332) is a theme folder in the plane's local folder, read by the same
engine (`theme::drafts`) and drawn only in its chat's workspace while it holds every hard floor
(primary text, the focus ring and the terminal's words at 3:1, as drawn, in every tint); the
window's draft strip (`ThemeDraftStrip.tsx`) has Keep, Revert and Compare, and the revert key
is ⌘⇧⌫ (Ctrl+Shift+Backspace elsewhere). The hard floors cover colour only; the revert key is
the way out of a draft whose face, size or spacing makes the window hard to read.
`purlis docs show themes` has the JSON's shape. **`purlis theme import <file>`** (#2334)
makes a theme folder from a VS Code colour theme, a base16 or base24 scheme, an iTerm2 colour
preset, a Ghostty theme or a theme folder's zip: the engine's `import` maps each kind through a
fixed table (`theme::table`, printed in `purlis docs show themes` and held to it by a test),
lists every source key no token takes, and the folder is put in place only when the engine
draws it, never over one that is there without `--force`. **`purlis theme export <folder>`**
writes the folder as a zip that `import` gives back file for file. **An extension's theme says what is wrong
with it** on its row in Extensions, beside the row's other complaints: why the engine refused
it, or what it left out.

### Theme levels: where themes and choices are kept

Themes live at three levels besides the built-ins and extensions (spec #2313, #2323), each a
reserved `themes/` folder of theme folders:

| Level | Where | Committed? |
|---|---|---|
| The person's own | `themes/` in purlis's config folder (beside `machine.json`) | never |
| The project's | `themes/` at the plane root | by `purlis save` |
| A workspace's | `workspaces/<ws>/themes/` | by `purlis save` while LIVE; kept in the clone while LOCAL |

A theme folder is picked by its folder's name. **A choice reaches its own level and the levels
farther out, never a nearer one**: the machine default names the person's own folders only; the
project's choice and committed default the project's and the person's; a workspace's choice and
default the workspace's, the project's and the person's. Within that reach the nearest level
that has a folder of the name is read, and a folder `extends` by the same rule. Each `themes/`
folder is opened from its level's root one folder at a time refusing a link, and its folders,
`theme.json` and icon set are read from those handles (`crates/purlis-core/src/theme/folders.rs`).
A folder named on the command line (`purlis theme check <folder>`), the person's own
`theme.json` in their config folder, and `purlis theme export <level id>` read the same way: the
folder is opened once refusing a link, and every file is read from its handle
(`crates/purlis-core/src/theme/opened.rs`). A theme folder's name and every name inside it hold
none of `< > : " / \ | ? *` nor a control character, and are no name Windows keeps for a
device, so a theme made on one platform opens on every other; import, export and the folder
reader each refuse such a name in one sentence (`crates/purlis-core/src/theme/portable.rs`).

**The precedence**, nearest first; at each level the person's choice beats the committed
default (`purlis_core::theme::resolve_levels`):

1. the person's choice for this workspace on this machine;
2. the workspace's committed default (LIVE workspaces only): `settings.theme.use`,
   `settings.theme.appearance` and `settings.theme.options` in its `workspace.json`;
3. the person's choice for the project;
4. the project's committed default: `[theme] use`, `[theme] appearance` and `[theme] options`
   in its manifest;
5. the person's machine default, else their own `theme.json`;
6. Purlis (Purlis Classic until the switch-over), or the first theme in force from an extension
   the project has on.

The plane root resolves from level 3 down. A choice names a theme and, optionally, an
appearance: an appearance's id, or `system`, which follows the operating system with any theme
that has a dark and a light appearance. A choice that names no folder within its reach nor any
other theme is passed over with a sentence, and the next level's is used. A choice of a theme
that is there and cannot be drawn (a refused folder, an extension's theme that is off) draws
Purlis with a sentence saying why.

**The person's choices** are made in Settings › Theme ([the gallery](#settings--theme-the-gallery)),
at the You level (the machine default), the Project level and each Workspace level, and kept in
`theme-choices.json` beside
`machine.json`, keyed by the project's id (`[project] id` where the manifest keeps one, so a
re-clone finds them; otherwise the id purlis holds the project by, which a re-clone elsewhere
does not find), never committed. Each project's entry keeps when it was last changed, and at the
file's 64 KiB cap the projects changed longest ago are dropped first:

```json
{
  "version": 1,
  "machine": { "theme": "dusk", "from": "machine", "appearance": "system" },
  "projects": {
    "<project id>": {
      "theme": "purlis-classic", "appearance": "light",
      "workspaces": {
        "alpha": { "theme": "brand", "from": "project", "options": { "accent": "teal" } }
      }
    }
  }
}
```

A choice is `theme` and, optionally, `appearance`; **`from`**, for a theme folder, the level of
the folder it picked (`workspace`, `project` or `machine`, #2516), so that level's folder of the
name is drawn and a folder of the name at another level never is; and **`options`**, the value
of each of the theme's options the person chose, by the option's id (#2527). A choice with no
`from`, written before choices named a level, reads the nearest level within reach that has the
folder, as before, and gains its level the next time it is written. The theme a level chose is
drawn with that level's option values, built onto the theme before it is drawn
(`Theme::with`, then the consent floors), and an option it names no value for draws the theme's
default: the values are the choice's own, as its appearance is, never another level's. A value
the theme does not have is said when it is drawn and dropped, with a sentence, the next time the
choice is written; choosing another appearance of the same theme keeps the values, and choosing
another theme starts from its defaults, and re-choosing a theme drops the values it no longer
has. Both choices are compared as written, so a choice with no `from` is the folder it reads now.
A `from` that names no level leaves the choice out, and
a value that is not an id is left out of it.

**A committed default carries option values too** (#2707): `options` names a value of each
of the picked theme's options by the option's id, as a choice's does, and is drawn the same
way, onto the theme before it is drawn, so the consent floors hold over every value. The
person's choice beats it, nearest level first, and a choice's values are its own: a person's
choice of the same theme with no values draws the theme's defaults, not the team's. Options with
no `use` choose nothing; an option or a value that is not an id is left out when read and
refused when saved. In Settings › Theme a committed default's value is badged Team, and setting
an option where it is drawn makes it the person's choice, starting from the team's values: those
values are copied into the person's choice, so later changes the team commits no longer reach
them there while that choice stands.

```toml
[theme]
use = "brand"
options = { accent = "teal" }
```

`purlis theme list` shows each level's choice after the themes, nearest first, with the level
of the folder it reads, its appearance and its option values, and `purlis theme check` names the
levels that chose the theme it checks (`purlis docs show themes`).

A project's `charter.local.toml` `[theme]` pick, and a LOCAL workspace's `settings.theme`, are
the person's choice where that file holds none, and are copied into it once, with a Notice. A
LIVE workspace's pick is its committed default; where it now comes before the person's choice
for the project, which the local file's pick used to beat, the Notice says so once. A theme
pick in a manifest is a proposed change when a chat makes it (ADR 0098 §4).

**Saving a theme folder repaints the window** (#2324, ADR 0098): the core watches the theme
folders the window's theme is read from — each level's `themes/`, its theme folders and the
folders directly inside them, and the folders of an extension that contributes only themes —
each non-recursively and never through a link, at most 128 for a window, found from the roots
the window's levels already hold (`purlis_core::theme::reload`). A save tells the window to ask
for its theme again, the one way it is ever drawn, so the consent floors and a project's font
approvals hold on a reload as they hold at launch. A burst of saves is one redraw, and redraws
are at least 400 ms apart, so the window is drawn again within a second of the last save. Where
the platform will not watch a folder, the core looks at it every two seconds instead. A theme
the engine refuses on a reload draws Purlis, with a Notice saying why; a project's font file
that changed since the project was approved is refused until the project is approved again.
That Notice offers **Review and allow** (#2647) when the engine says the approval is all that
stands in the way (`Drawn::awaits_approval`): it shows the project's trust ask in place, what
changed first and then its font files by theme folder, and a yes approves exactly what the ask
showed and draws the theme again. A file changed after the ask was drawn approves nothing; a
no leaves the theme refused. The press is the person's alone. It is offered only when what the
project asks for again is font files alone; when it asks for more, or was never approved on
this machine, the Notice has no button and says to close the project and open it again, where
its whole trust ask is shown. The ask names the themes and workspaces the font files belong
to, and says that the system's font engine reads them. Whether to offer it is found once for
each set of faces a refusal waits on, so a redraw that changed no face reads nothing again.

**A workspace switch repaints with a short crossfade**: colours ease across `duration.enter`
with `easing.standard`, from a rule in the base layer at no specificity, so a surface with a
transition of its own keeps it; reduced motion makes it immediate.

**An old single-file `theme.json`** keeps working and is offered once, in the Inbox, a move into
a theme folder of the person's own (`themes/<name>/theme.json`), chosen as the machine default
where none is chosen, so the window draws what it drew. The move is all or nothing, and only the
person's press in the window makes it.

### Settings › Theme, the gallery

**A card per theme a choice at the level may name** (spec #2313, #2331,
`purlis_core::theme::gallery`): Purlis Classic, the theme folders within the level's reach
(nearest level first, each card saying its level, so two folders of one name at two levels are
two cards a choice tells apart, #2516), and the themes of the extensions in force there. You is the machine's level (its
choice is the machine default), Project the project's and Workspace the workspace's. The
person's own single-file `theme.json` is no card, since no choice names it; when it is what is
drawn, the gallery says so, with what following the system does with it: "Drawn here: your own
theme.json, following the system", or that it stays in its one appearance (#2615).

- **Each card shows the theme specimen in each of its appearances** (up to three, then a
  count): the specimen #2330 draws for a chat's `theme_preview`, asked by the window with
  `theme_specimen` and the level, one at a time, and kept by the card's `stamp`, which changes
  whenever anything the theme draws does. A card of the theme chosen at the level is pictured,
  as it is drawn when pointed at, with the option values chosen there. **Only the cards on
  screen are pictured** (#2614): a card's pictures are asked for once it scrolls into view, and
  one still waiting its turn when its card scrolls away is not asked for. **At most 48 pictures
  are kept in all** (`MOST_KEPT`); past that, the one shown least recently is let go and asked
  for again if its card comes back, and one a card on screen shows is never let go.
- **A chat's `theme_preview` goes first** (#2614, `themepreview::turns`): one specimen is drawn
  at a time. A chat asked while a card's picture is drawn waits for it, and that picture gives
  way, so the chat is drawn next. While a chat holds or waits for the painter, the app answers
  the window `SPECIMEN_BUSY`, and the window asks again every second (`BUSY_WAIT_MS`), for as
  long as a chat's whole ask takes and while the card is on screen; a picture that could not be
  drawn is never kept. **The gallery is owed a picture after a chat's turn** (#2751): when the
  window asked during it, every chat is answered busy until the window has taken the painter
  once, or a specimen's budget has passed, and that picture does not give way. So however many
  chats loop `theme_preview`, the cards on screen get their pictures between their turns.
- **Pointing at a card, or moving to it with the arrow keys, draws its theme in the window**,
  and leaving puts the person's theme back (`theme/hoverPreview.ts`). The engine resolves the
  card in the appearance a choice of it would draw (`theme_card_drawn`), with the consent floors
  held, and nothing is written. A theme the window draws while a card is shown is what goes
  back; a move from one card to the next never shows the person's theme between them. The
  pointer and the keyboard share one preview: it goes back when neither is on a card, when the
  window is hidden or loses the focus, on Escape, and once the cards are out of the document (a
  project switch). An answer that arrives after its card was left, pressed or closed draws
  nothing, and a card whose theme is the one drawn draws nothing either.
- **Pressing a card, or Enter or Space on it, writes the person's choice at the level**, and
  the row offers Undo. **Use the default** takes the choice at the level away, so the levels
  underneath decide again. A line over the cards says what is drawn here, with the source badge
  the rest of Settings uses: the person's choice, or the level that decided. Appearance is
  offered only for a choice made at the level, so picking one never quietly makes a choice. A
  refused card chooses nothing and says why as its description.
- **Under the cards**: the chosen theme's appearances (one by id, its first, or Follow the
  system, which says when a theme of one tag stays in it); its options, each a choice of its
  values, or Theme default to choose none, kept in the person's choice at the level
  (`theme_choose_option`, #2527), with a badge
  naming the level whose choice made the value, or the theme's default (with no choice at the
  level yet, the choice that draws the theme there becomes the person's, with the value); the
  theme check's findings, in the words `purlis theme check` prints
  (`theme::warnings_said`), with what was left out of the theme and the consent floors it
  breaks; and **Duplicate as my theme** and **Open folder**.
- **Duplicate as my theme** makes a theme folder of the person's own whose `theme.json`
  `extends` the card's theme and is called "<name> (mine)", under the first of `<id>-mine`,
  `<id>-mine-2`, … that no folder has. A folder of the person's own reaches Purlis Classic and
  their other folders, so those are the cards it is offered on. Every step to the new folder is
  opened refusing a link, and nothing is written over.
- **Open folder** opens the card's theme folder in the file manager: the folder the engine
  places for the card, never a path the window sent.
- **The keyboard and a screen reader**: the cards are a listbox, one Tab stop, the chosen card
  the stop at rest (`roving.ts`); selection does not follow the focus, so moving through the
  cards writes nothing. Nothing in the group takes the focus by itself.
- The machine default, an option's value, a duplicate and Open folder are the window's alone
  (`WINDOW_ONLY`); the gallery's reads are served on a link, as `theme_in_force_at` is: the
  theme in force with no window, which moves no window's theme watch (#2697).

### Where a user's theme lives

`$PURLIS_CONFIG_HOME`, else `$XDG_CONFIG_HOME`, else `~/.config` — then `charter/theme.json`.
That is `machine.rs`'s ladder, rung for rung, and the argument is the one ADR 0040 made
for pins: **a theme is how one operator likes their window, not a fact about the plane.** A
theme committed to `charter.toml` would arrive with every clone and repaint somebody else's
window in colours they never chose.

**It is a file beside `machine.json`, not a fifth thing inside it.** `machine.rs` says four
things and nothing else, and says the count is load-bearing; `purlis report`'s publish consent
is already a second file in that same directory, so a third is the shape the directory already
has. Nothing about this needs ADR 0040 amended again.

**It is read before the window exists and handed to it as it is created** (M6.7), the same way
as the layout below, so an operator with a theme of their own never sees the built-in painted
first. The theme engine (`purlis_core::theme`) judges it, and whatever makes it refuse the theme — a
misspelled token, a value that is not hex, a file that is not JSON — is said in the Inbox,
as a Notice about this machine, with the file's path. **It wins over an extension's theme**: it
is the one theme the operator wrote for this machine themselves, so an approved extension's
contribution does not repaint over it. Delete the file to have the extension's. The Inbox's
row about it offers **Use built-in…** (NO-6 #1238), which asks first and then moves the file
aside to `theme.aside.json` (or the next free `theme.aside-N.json`), never over another file,
and draws what is in force without it.

**A theme switched while the window is up reaches the terminal too.** The stylesheet follows by
itself — `apply` rewrites the custom properties it reads — but xterm was handed an object when a
pane was built. `theme.ts`'s `onDrawn` is the change signal, and each pane hands its terminal the
new object through `pane.options.theme`, which is xterm's own way to retheme a live terminal.

### The window's layout is a file, handed to the window as it is created

A theme and a **layout** — which regions are drawn, on which side, in what order and how big
(`app/src/regions.ts`) — are both "how one operator likes their window", and both are files
beside `machine.json`: `$PURLIS_CONFIG_HOME` (else `$XDG_CONFIG_HOME`, else `~/.config`), then
`charter/layout.json`. It is not a field of the machine store: ADR 0040 amended ADR
0034 for _"how the operator arranged what this file already names"_, and a region arrangement
names nothing that file holds.

**It is a file because a file is the one form an operator can hand-edit**, and a region's `side`
and `order` have no control in the window yet. **It is injected, not fetched, because of the
first frame.** A layout has no stand-in the way the built-in theme does: the operator's
arrangement _is_ the thing, and one read over an asynchronous Tauri command would land after
the window had painted the default and make it lay itself out again — the flash
purlis#141 left. So the Rust side reads the file before it builds the window
(`purlis_core::windowprefs`, `app/src-tauri/src/windowprefs.rs`) and puts what it read into
the page with the window's `initialization_script`; the page's first render is drawn from it
and nothing is fetched. What the window changes — a region put away, a slot dragged — is
written back to the file through a command, pretty-printed, `0600`.

Web storage held the arrangement before this, under `charter.layout`. The first launch that
finds no file draws from that value, moves it into the file, and removes the key once the file
has it; nothing reads the key after that.

#### The format, for editing it by hand

```json
{
  "version": 2,
  "regions": [
    {
      "id": "navigation",
      "side": "right",
      "order": 0,
      "collapsed": false,
      "size": 22,
      "view": "explorer"
    },
    { "id": "aside", "side": "left", "order": 0, "collapsed": false }
  ],
  "projects": {
    "/home/me/project": {
      "regions": [{ "id": "navigation", "side": "left", "order": 0, "collapsed": true }],
      "explorer": { "closed": ["files"], "folded": ["alpha/svc"] },
      "chats": { "scope": "all" }
    }
  },
  "text": { "window": 15, "terminal": 14 },
  "editor": "zed",
  "chats": { "grouped": true, "tabbed": true, "away": false },
  "notifications": { "quiet": { "on": true, "from": "22:00", "to": "07:00" } },
  "explorer": { "closed": ["repos"] },
  "rail": "auto",
  "dismissed": { "/home/me/project": ["pin-dormant:ide", "chat-fresh:3"] }
}
```

- **`version`** is `2`, what purlis writes since #1673. A version 1 file, which every purlis
  before it wrote, is read too and moved forward at the first change: its arrangement becomes
  the machine's, every project starts from it, and the explorer's region is the navigation
  region (below). A file with no version, or another one, is not read: the window is drawn in
  the default arrangement and the Inbox says why.
- **`regions`** lists placements: **the machine's arrangement**, what a project with none of its
  own starts from. Changing the arrangement in any project writes it here too, so a project
  opened afterwards starts as the person last left one. The ids this build has are `navigation`
  (the left side: the Chats, Explorer and Search views; version 1 called it `explorer`,
  and that id is still read as this one), `aside` (the attention region: the Inbox, Personas
  and Vaults views and the extensions' panels, since #1678 and #1738), and the Panel's two
  halves (#1737): `panel-left` (the Repos and Changes views) and `panel-right` (the Memory,
  Todos and Sessions views, #1738). Changes was a view of `navigation` until #1739, and Memory,
  Todos and Sessions of `aside` until #1738: such a `view` written there is read past without a
  word, and the side opens on its default. A region
  the list leaves out is where it starts, so a file written before #1737 opens the Panel once, at
  its default height; an id this build does not have is left out and named in the Inbox, once,
  and the next change rewrites the file without it, which is how an older purlis reads this
  one's Panel. **`bottom`**, the repository state bar every file before #1676 placed, is read
  past without a word: its content is the Changes view.
- **`projects`** is what each project keeps on this machine, by its path (#1673, B-11).
  **`regions`** is its own arrangement, as the top-level `regions` is written: the side a view
  is on, which view it shows, its width and whether it is away come back as that project left
  them. Leave it out and the project starts from the machine's. Beside it, what its views keep
  (#1686, #1696):
  - **`explorer`**: **`closed`**, the sections folded on their headings, as the top-level
    `explorer.closed` says them, an empty list when every one is open; and the rows of
    Explorer's trees as the person left them: **`folded`**, the clones folded under *Repos
    and branches*, **`opened`**, the folders opened under *Files*, and **`shut`**, the
    cockpit's *Files* rows closed. These three are Explorer's own row keys, written by it and
    not meant to be written by hand; a key that names nothing on screen opens or folds
    nothing. The whole of `explorer` is held to 2 KiB, letting go of the folders opened
    longest ago first.
  - **`chats`**: **`scope`**, the Chats view's scope, `tab` (This tab) or `all`. Leave it out
    for the workspace's, the default. A pick an older purlis kept in the window's web storage
    is moved here the first time the Chats view is drawn. **`folds`**, the rows folded or
    opened by hand, by each chat's id (`true` is folded), so a relaunch draws the list as it
    was left (#1687). At most 48 are kept: past that, the folds of chats that are not open go
    first, the ones kept longest ago first. A chat that has no id has its fold kept for the
    window's run only.
  - **`inbox`**: **`scope`**, whose asks the Inbox lists: `all` (All projects) or `workspace`
    (This workspace). Leave it out for the project's, the default. A value that is neither is
    read as the default.

  purlis writes a project's entry when anything in it changes; the core keeps the entries a
  window did not send, so two windows keep each other's. It keeps at most 32, and at most
  24 KiB of them, letting go of the projects opened longest ago first. Should the whole file
  still pass its 64 KiB, other windows' projects go first, then the writing window's, whole
  entries each time; a write that would pass it with no project left is not made, and the
  file on disk stays. An entry that is not
  an object, or whose `regions` is not a list, is skipped, and the project starts from the
  machine's.
- **`view`**, on a region that has views, is the one it shows: `chats`, `explorer`, `search`
  or `changes` for `navigation`, `inbox`, `personas`, `vaults` or `panel:` and an extension's
  panel key (`panel:ext/<extension>/<id>`) for `aside` (#1678), `repos` for `panel-left`, and
  `memory`, `todos` or `sessions` for `panel-right` (#1738).
  Leave it out for the one it opens on: `chats` on the left, `inbox` on the right, `repos` and
  `memory` in the Panel. A view the region does not have opens on that one, and the Inbox says
  so — but a view another region of the same side holds now, as `aside` held `memory` before
  #1738, or the place a view was moved to (`moved`, below), opens on it without a word; an
  extension's panel
  that is not contributed now opens on it too, and is kept, so it comes back when its extension
  does.
- **`side`** is `left` or `right` — the two slots beside the terminals (#1676: a region a file
  still puts on `bottom` goes back to its own side without a word, and its size, a height
  there, is left out). Two regions on one side stack in **`order`**, lowest first; a tie is
  broken the same way at every launch. A half of the Panel is its own side's for good: its
  `side` is not read.
- **`collapsed`** puts a region away when it is `true`, and only then. Anything else draws it.
  A region with views keeps them, hidden, and its activity bar stays at the window's edge.
- **`size`** is how big the region's slot is, as a percentage of the window's width, above 0
  and at most 100; leave it out for the default. Keep it inside the slot's own bounds, which a
  drag is held to as well: the left slot is 8–45%, the right 10–45% (`SLOTS` in `regions.ts`). The left slot is also never
  narrower than 11rem, whatever percentage that is of the window (#1499): under it a nested row
  of the Chats list has no room for its state, and the floor follows the text size. On a half
  of the Panel it is the **Panel's height**, as a percentage of the regions' height: 30 by
  default, at most 70, and never under 120px (#1737, P-12); a double-click of the Panel's
  handle leaves it out again.
- **`share`**, on `panel-left` only, is the left half's share of the Panel's width, as a
  percentage (#1738, P-12): 50 by default, and either half at least 20, whatever the number
  says; a double-click of the handle between the halves leaves it out again. With one half
  away, the other has the whole width whatever `share` says.
- **`moved`**, on a region that has views, lists the views moved into it from its side's other
  place (#1740, P-3): a sidebar's from its half of the Panel, a half's from its sidebar, in the
  order its bar draws them, after its own. A view listed there is not in the place it came
  from, and goes back to it when it is moved back; leave `moved` out where nothing moved in.
  An extension's panel can be listed on `panel-right` only, the half of the side that takes
  them. A view of the other side, the Inbox (P-4: pinned to the right sidebar), or a name this
  build has no view for is left out and named in the Inbox; a region's own view in its own list
  is read past. _Use the default layout_ puts every view back where it starts.
- **`text`** is the two text sizes, in px (purlis#283): **`window`**, the root font size
  every `rem` in the stylesheet is measured by, and **`terminal`**, every chat's terminal. Each
  is a whole number from 10 to 24; leave one out for its default, 14 and 13. A size that is
  not one is its default, and the Inbox says so. Settings and the size keys
  (`⌘`/`Ctrl` with `=`, `-`, `0`, `app/src/textSize.ts`) write it; it is in this file and not in
  a plane because a size is this machine's, and a plane would carry it to every clone.
- **`appearance`** is what the person sets that every theme honours (#2326, #2925):
  **`density`** (`comfortable` or `compact`), **`fonts`** (a family for `ui`, `mono` and
  `terminal`), **`motion`** (`system` or `reduced`), **`contrast`** (`standard` or `more`) and
  **`colours`** (a colour for each token the person sets, `#rgb`, `#rgba`, `#rrggbb` or
  `#rrggbbaa`). Leave any out for its default; Settings › You writes them. A value the engine
  cannot use is its default, and the Inbox says so.
- **`editor`** is your editor (RC-20, ADR 0081 §3), where *Open in your editor* sends a file
  at a line: `vscode`, `zed`, `idea` (a JetBrains IDE) or `variable` (`$VISUAL`, else
  `$EDITOR`, from purlis's own environment, run with `+line` and the file). Leave it out and
  none is chosen: *Open in your editor* asks for one. Any other value is none, and the Inbox
  says so. Settings writes it (`app/src/yourEditor.ts`). It is a word and never
  a program: the core builds the URL, or reads the variable itself, so nothing written here is
  run.
- **`chats`** is how chats are listed and summed up, on this machine (#1499, V100-73):
  **`grouped`**, `true` when the sessions of one workspace stand together. A row is one line
  (#1675), so the `lines` an older purlis wrote here is read as if it were not there. And what pressing a
  task does (#1489, V100-74): **`tabbed`**, `true` when it opens in a tab of its own, with a
  minimise button in place of the close; `false`, its session's tab is switched to it. And
  **`away`** (#1514, V100-73), `false` when coming back to the window draws no summary of what
  happened while you were away. And **`beside`** (#2024), `true` when a browser a chat starts
  opens beside the chat, in its tab. Leave any out for its default, `false`, `false`, `true`
  and `false`; purlis leaves the whole of `chats` out
  while all are the defaults. A value that is neither is the default, and the Inbox says so. Settings
  writes it (`app/src/chatsListPrefs.ts`). It is in this file because it is how one person
  likes their window, and a project would carry it to every clone.
- **`notifications`** is when the asks' system notifications keep quiet, on this machine
  (#1989): **`quiet`**, with **`on`**, `true` while quiet hours are kept, and **`from`** and
  **`to`**, the times they start and end, `HH:MM` on a 24-hour clock and this machine's own
  time. A `to` before the `from` runs across midnight; the same time for both is no quiet hours.
  In them no notification is shown and an ask waits in the Inbox; when they end, one
  notification names the chats still waiting on you.
  Leave any out for its default, `false`, `22:00` and `07:00`; purlis leaves the whole of
  `notifications` out while all are the defaults. A value that is not one is the default, and
  the Inbox says so. Settings writes it (`app/src/notificationPrefs.ts`), and the app reads it
  each time it would show a notification (`app/src-tauri/src/asknotify.rs`).
- **`explorer`** is Explorer's folded sections, on this machine (#1677): **`closed`** lists
  the sections folded on their headings, out of `repos` (the focused workspace's
  repos and branches) and `files`. Leave it out, as purlis does, while every section is open. A
  name that is not a section is left out, and the Inbox says so; `workspaces`, a section Explorer
  had until the strip became the one place a workspace is selected, is left out without a word.
  Folding a section
  writes it (`app/src/explorerSections.ts`). Each project keeps its own too, under
  `projects` (#1686); this one is the last fold in any project, which a project with none of
  its own starts from, as a project with no arrangement starts from the machine's.
- **`rail`** is whether the project rail hides itself (#2915): `"auto"` while it does, and
  left out while it is always shown, the default. Any other value is the default, and the Inbox
  says so. The rail's own last button and the title bar's show button write it
  (`app/src/railMode.ts`); the first frame draws the rail as it says. Where the rail would
  crowd the work, the window hides it over a `rail` left out (#3077, `app/src/railRoom.ts`), and
  that hide is never written.
- **`dismissed`** is the Notices you dismissed (NO-2, V91j), per project by its path, each
  by its cause. A Notice stays hidden while its cause is kept here, and the window lets the
  cause go once the project answers without it, so the Notice shows again if the cause comes
  back. Only causes the core answers for are kept: `pin-dormant`, `chat-resumed`,
  `chat-guessed` and `chat-fresh`; any other is left out, as is anything past 200 per project
  (`app/src/dismissals.ts`). A resumed chat's cause names the conversation it was resumed by
  (`chat-resumed:3:<conversation>`), so another conversation's note shows. The core writes
  this field one project at a time, under the file's lock, and a layout write from a window
  never changes it, so two windows keep each other's. It takes at most half the file: past
  that, the projects opened longest ago lose theirs first. It is here and not in a project
  because what you have already seen is yours on this machine. Under the key
  **`"on this machine"`**, in place of a project's path, are the Notices shown once per
  machine whatever the project: `chip-explained`, the Notice the first dispatch shows to explain
  a tab's chip of tasks (#1501). The core keeps only those causes there, whoever writes the
  key and whatever was hand-edited into it, adds one under the file's lock so two windows keep
  it once, and the window asks the file again before it shows such a Notice. Nothing lets them
  go, and the projects' dismissals never push them out. Delete it to see every Notice again.
- **The file is read once, as the window is created.** Edit it while purlis is not running,
  or expect the next change made in the window to replace your edit.
- **Nothing in it can stop the window.** A file that is not JSON, is not a layout, is a link or
  is over 64 KiB is refused whole and said in the Inbox; a field that is wrong costs only
  that field. The next change made in the window rewrites a file that did not parse — the Inbox
  says so — but never one purlis could not read at all (a link, a FIFO).

## Motion is data too

**How long a change in the window takes, and how it moves, is a theme token — named for what the
motion is for, read by name, and collapsed in one place for an operator who asked for less.**
This is M7.2, and it is the colour layer's argument applied to time.

Before it, the window had two motions — `900ms linear` on the spinner and `120ms ease` on the
explorer's twisty — each spelled out where it was used and each with its own
`@media (prefers-reduced-motion)` block beside it. That is where colour was before this file
existed: every value a local decision, every guard something the next component had to remember.
The operator has asked for animation twice (_"no icons, visual components, animations"_, _"show
pipelines with animation"_), and adding it on top of that shape would have multiplied both.

### The vocabulary

`DURATIONS` and `EASINGS` in `app/src/theme/motion.ts`. Fourteen names, written to the document
as `--motion-duration-*` and `--motion-easing-*` beside the colours. The built-ins are Purlis
Classic's; Purlis draws `duration.quick` at 150 ms and `easing.standard` and `easing.enter` on
the shell's one ease (below):

| token              | built-in           | what it is for                                                          |
| ------------------ | ------------------ | ----------------------------------------------------------------------- |
| `duration.quick`   | 120 ms             | the window answering a press: a tab lit, a chevron turning              |
| `duration.enter`   | 160 ms             | a surface arriving: a menu, a popover, a dialog, a region brought back  |
| `duration.settle`  | 280 ms             | a mark arriving at an answer: a pipeline finished, a chat changed state |
| `duration.spin`    | 900 ms             | one turn of _still running_                                             |
| `duration.breathe` | 1600 ms            | one breath of _queued, not yet running_                                 |
| `duration.slide`   | 260 ms             | the rail hiding and showing; a workspace switch sliding its regions in  |
| `duration.reveal`  | 220 ms             | the rail's peek coming in from the window's edge, a bottom half rising  |
| `duration.leave`   | 200 ms             | the peek going, and the rail fading out as it hides                     |
| `duration.open`    | 160 ms             | a panel opening beside the work: the right panel (Purlis 240)           |
| `duration.swap`    | 0 ms               | one side view's list taking another's place (Purlis 180)                |
| `easing.standard`  | `[0.2, 0, 0, 1]`   | a state changing in place                                               |
| `easing.enter`     | `[0, 0, 0.2, 1]`   | decelerating into place, which is what reads as _arrived_               |
| `easing.steady`    | `[0, 0, 1, 1]`     | a turn; an eased one stutters once a second                             |
| `easing.breathe`   | `[0.4, 0, 0.6, 1]` | a loop with no visible seam                                             |

**The shell's motion rule** (R150, #2925): one ease, `cubic-bezier(.22, .8, .24, 1)`, which
Purlis gives `easing.standard` and `easing.enter`, and 150 ms for a chevron and a switch's knob,
Purlis's `duration.quick`. Each of the shell's other steps is a duration token that arrives with
the motion it times: `slide`, `reveal` and `leave` came with the project rail (#2915), at the same
values in both built-ins; `open` and `swap` came with the four corners (#2917), and a bottom half
rises over `reveal`. Each corner motion plays only on the part that just appeared. Nothing leaves
with motion there: a part that closes is gone. A workspace
switch slides the regions in on `slide` too (#2916): 28px from the side the switch went towards,
fading in, with the activity bars, the rail and the title bar still.

**Semantic, not a scale**, for the reason `needs-you.base` and `danger.base` are two tokens: a
theme that wants arrivals quicker changes `duration.enter` and does not also speed up whatever
happened to share a rung of `duration-200` with it.

A theme file carries them under `motion`, beside `tokens`:

```json
{
  "name": "brisk",
  "appearance": "dark",
  "motion": {
    "duration.enter": 90,
    "easing.enter": [0, 0, 0.1, 1]
  }
}
```

- **A duration is a whole number of milliseconds, 0 to 10,000; an easing is the four numbers of
  a cubic Bézier**, each `x` in 0..1 and each `y` in -1..2. Numbers and never text, which is the
  same security boundary as hex-only colour: `motionVariables` writes the CSS from the numbers,
  so `"150ms; } body { display: none"` has nowhere to go (ADR 0041's parse-and-re-emit). The
  `x` range is CSS's own rule — a curve that breaks it drops its whole declaration, so the motion
  would silently vanish — and the `y` range leaves room for an overshoot and none for a curve
  that flings a menu off the window.
- **A partial `motion` is normal, and so is none.** What a theme does not name moves the way the
  built-in of its appearance does; a bad value refuses the theme and is reported, exactly as a
  colour does.
- **`0` turns a motion off.** A theme that sets every duration to zero is a still window.
- **An extension restyles motion the same way it restyles colour** — through the theme it
  contributes (ADR 0041; the panel contract of ADR 0043 is the same registry). Nothing
  about the extension path changed: the theme text goes to the theme engine's `load`, which
  reads `motion` with the colours.

### Reduced motion is the layer's job, once

Under `prefers-reduced-motion: reduce`, or with **Reduce motion** on in Settings › You › Motion
and contrast (#2925, layered on the system's: either one is enough), `drawIn` writes **every
duration as `0ms`**, and follows the setting live if the operator changes it while the window is
up. It also writes `data-motion="reduced"` on the root, which **one global switch** reads
(`app/src/theme/reduced-motion.css`, R152): no transition, and every animation plays its whole
run at once, so a motion written without a token stops too, and a component waiting for an
animation's end still hears it. xterm blinks its cursor on a timer of its own, so under reduced
motion every terminal is told not to blink, whatever the theme says. The switch's declarations are important in the first layer,
so no later layer turns motion back on. A transition over no time is
no transition, and a looping animation with a zero duration has a zero active duration (Web
Animations), so a spinner does not spin and a pulse does not pulse. **A component written
tomorrow gets this for free, because it reads a token** — and `literals.test.ts` refuses a
`prefers-reduced-motion` query anywhere outside `src/theme/`, so there is no second place that
could mean something different by it.

It follows that **every looping mark is designed to read standing still.** A spinner at rest is
the loader glyph; a breathing mark at rest is the mark at full weight. Motion is decoration on a
meaning that the shape and the word already carry, never the meaning itself.

Collapsing to zero rather than swapping movement for a cross-fade is a decision, and the cheaper
of the two: WCAG 2.3.3 asks that motion _can be disabled_, and a cross-fade layer would need a
second set of keyframes per motion. The seam for it, if it is ever wanted, is `motionVariables`.

### What moves, and what is deliberately still

The rule is **motion explains a change of state; it never decorates one.** Every motion is in
one labelled section at the end of `App.css`.

| moves                                      | how                                                      | why                                                                                                     |
| ------------------------------------------ | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| a tab being selected, on all three strips  | its surface and lit edge cross-fade, `quick`             | the change the operator just made, answered; hangs off `[data-strip]`, so a restyled strip keeps it     |
| a strip starting to collapse into `N more` | the button fades in, `enter`                             | says the tabs went somewhere; not replayed as the count changes on resize                               |
| a popover or menu opening                  | fades in a quarter-rem out of its anchored side, `enter` | says what opened it; hangs off Radix's popper wrapper, so the next popover gets it                      |
| a dialog and its scrim                     | fade, no movement, `enter`                               | a question should appear where the eye already is                                                       |
| a region brought back                      | fades in, `enter`                                        | only its opacity; see below                                                                             |
| a running pipeline                         | spins, `spin`                                            | _still happening_, which amber alone cannot say                                                         |
| a queued pipeline                          | a clock that breathes, `breathe`                         | alive and not working; a spinner would claim work being done                                            |
| a pipeline that finishes on screen         | its new mark grows into place once, `settle`             | the answer arriving                                                                                     |
| a chat's state mark                        | colour, shape and ring morph, `settle`                   | the state changing, rather than blinking                                                                |
| a chat that starts waiting on you          | its ring knocks twice, `settle`                          | the one signal this app exists for, arriving                                                            |
| the same, on a row of a list               | its hand knocks twice, `settle`                          | a row's mark is the hand and has no ring; only on the change, as the ring                               |
| a row of the Chats list that was asked for | the hovered row's band for a moment, faded in once, `settle` | where a press elsewhere went; the band is there, still, under reduced motion                            |

**Nothing animates because it mounted.** A mark that animates on a change has to tell a change
from a first draw, and CSS cannot: an animation plays when its element appears. Without that, a
"settle" would play on every finished pipeline at launch, and every waiting chat would knock each
time the workspace strip brought its tabs back into view. `useArrived` (`app/src/lib/arrived.ts`)
is how a component says _this value changed while I was on screen_, and the stylesheet only
animates what it is told arrived.

Left still, on purpose:

- **The terminal, always.** It is the product and it is what the operator is reading.
- **A region's size.** A slot that grew over time would refit xterm on every frame of it, which
  is terminal output moving; a region arrives at its full size and only fades.
- **A tab's width or position.** A tab that slides under the pointer breaks aiming (`tabs.ts`);
  the collapse into `N more` is instant, and only the button that appears is drawn arriving.
- **Closing anything.** Radix unmounts a closing surface at once unless an animation is running
  on it, and a close that lingers leaves a dismissed question over the window the operator has
  gone back to.
- **Counts and words** — the needs-you badge, the gauge, the status line. They are read, and a
  number that animates is a number that is briefly wrong.
- **Hover on rows.** Only a tab eases its hover, because a tab's hover and its selection are the
  same properties; a list of fifty rows fading under a moving pointer is decoration.

**Nothing delays input.** A dialog is focusable and answerable from its first frame; the fade is
only what it looks like. The longest one-shot motion is 280 ms and nothing waits on one.

### What it costs

Measured against `origin/main` with `vite build`: CSS **+2.80 kB (+0.62 kB gz)**, JS **+3.09 kB
(+1.06 kB gz)**. Cold start gains nine `setProperty` calls in `drawIn`, beside the fifty-four the
colours already make.

## Geometry and fonts

**Every size in the window comes from one scale, and a theme moves it only within bounds.**
Colour and motion
were tokens long before size was: in October 2026 `App.css` held 132 different paddings, 20 font
sizes and 11 corner radii, and nothing styled a bare `button`, `input` or `select`, so each one
WebKit drew was its own grey bezel. ADR 0094 records the fix and its reasons; this section is the
reference.

### Three layers, and who owns each

| layer                 | what it is                                                                                | where                                         | who sets it                     |
| --------------------- | ----------------------------------------------------------------------------------------- | --------------------------------------------- | ------------------------------- |
| **primitive palette** | raw colour steps a theme builds from (`teal.400`); private to that theme file             | a theme file, when one has a `palette`        | the theme                       |
| **semantic**          | what a colour means here: the [vocabulary](#the-vocabulary) above, plus the motion tokens | `app/src/theme/*.json`, written by `theme.ts` | the theme                       |
| **core geometry**     | spacing, type sizes, corners, heights, focus rings, elevation, fonts                      | `app/src/theme/geometry.css` (Classic's)      | a theme, within core's bounds   |
| **component**         | one control's look, as aliases of the two above (`--btn-bg: var(--control-base)`)         | the primitive classes (DS-2)                  | purlis                          |

**A theme sets geometry as numbers, within bounds** (ADR 0097, #2326). A theme folder's
appearance names a geometry token in its `tokens` with a number (`"radius.md": 6`,
`"space.4": 10`, `"line-height.body": 1.5`); the bound of each is in the table below, and a
value outside it, a fraction where a whole number is due, or anything that is not a number
refuses the theme with a sentence that names the bound. The core's theme engine
(`purlis_core::theme::geometry`) writes each custom property from the number, so a theme's text
never reaches a declaration; `geometry.css` holds core's values, Purlis Classic's, and the
engine's test holds what it writes for Classic equal to that file, value for value. A format 1
file names no geometry: one that does is refused and told it belongs in a theme folder. A
shadow's shape is two of these numbers per lift; its colour is the theme's `overlay.shadow`.

**Density is the person's, and every theme has both.** Settings › You › Density and fonts says
comfortable or compact, kept in the layout file beside the text sizes (`appearance`). Core
defines Purlis Classic's compact (the compact column below), and a theme tunes its own in a
`compact` object of geometry tokens, held to the same bounds, for every appearance. A token
the theme's `compact` does not name is the smaller of core's compact and the appearance's own
comfortable value where core's compact moves it, so compact is never looser than comfortable,
and the appearance's comfortable value where it does not. The engine resolves both geometries for
every theme, and the window writes the one the person's density picks.

**Fonts are woff2 files in the theme's folder, served by purlis.** A theme's `fonts` names
`ui`, `mono` and `terminal`, each a `family` (a label Settings shows) and one to eight `faces`,
each `{ "file": "fonts/<name>.woff2", "weight": 400, "style": "normal" }`. The file is one plain
name in the folder's `fonts/`: a URL, an absolute path, `..`, another folder or a link on the
way refuses the theme, and so do a file past 2 MiB and one without the woff2 signature. The
engine reads each file when it reads the theme and hands the window its sha-256; the window
asks purlis for the bytes by that digest (`theme_font`), and the engine reads the file again,
contained and capped, and answers only while it still has that digest. The window makes the face
with `FontFace` from the bytes and registers it under a name the engine made from the digests,
so nothing in the window is a path or an address for a font, the stylesheet's
`default-src 'self'` is unchanged, and the theme's family name never reaches a declaration.
The faces are asked for after the first frame: it is drawn in the family list's fallback, and
the text redraws when a face arrives. A theme not read from a folder (an extension's text) has
no `fonts/`, so naming a font refuses it. The person's own `theme.json` is in their config
folder, beside `machine.json`, and its fonts are that folder's `fonts/`.

**A face is read through the theme folder's handle.** For a theme folder of a level, `fonts/`
is opened from the handle the folder's `theme.json` was read through, refusing a link, and each
face from the handle of `fonts/`, refusing a link again; serving a face reads it from the same
handles. **A project's and a workspace's font files are approved with the project** (ADR 0098,
as amended 2026-10-18): the trust ask lists each by its theme folder, size and digest (at most
64 files and 32 MiB per project, 24 per folder), asks again when one is new or changed, and a
face from the project is drawn only when that approval holds it. The
person's own theme folders' fonts need no approval.

**The person's font beats the theme's.** Settings › You › Density and fonts takes a family for
the window, for code and for the terminal; an empty field draws the theme's. The engine holds
it to the same closed grammar as a theme's label and writes it quoted in front of core's family
for the slot (`purlis_core::theme::Person`), so a family that is not installed falls back to
core's. The terminal reads its family from the theme too (`terminalFont`), and its old literal
is core's terminal family.

**The person's colours are drawn over any theme, held to the floors** (#2925, ruling #2814·8).
Settings › You › Your colours takes a colour for any token of the vocabulary, one
`token #colour` a line, and the engine draws each over the theme in force, whatever it is
(`purlis_core::theme::Layer::over`). A colour that would put a pair of the check under its floor
in that theme, and lower than the theme has it, is left out there and the theme's is drawn; a
Notice names each one. The consent floors are held after the person's colours, as after a
theme's option values, so a colour can no more bring danger near the accent than a theme can.

**Minimum contrast is the person's too** (#2925, ruling #2814·7). Settings › You › Motion and
contrast offers the theme's floors or more. More is layered on the system's Increase contrast:
it draws a theme's high-contrast appearance where it has one, as the system's does, and it
raises the floors the engine holds, words from 4.5:1 to 7:1 (WCAG 1.4.6) and marks from 3:1
to 4.5:1 (`purlis_core::theme::Least`). A pair under its raised floor has its front drawn
lighter on a dark ground or darker on a light one, in small steps toward white or black, and
then its back the other way where the back is not one of the window's grounds. Where the back
has to move, the two may swap sides instead, dark words on a light mark, when that leaves fewer
pairs under their floors. Marks are raised before words. A grey tier is raised as far as the
tier above it and never past it, so muted words can at most match secondary ones and the tiers
keep their order. A floor under a mark's, a hint's, is not raised. The consent floors are held
at the raised floors, with purlis's own values raised where a theme's are put back, in the
theme as written and in every workspace tint. A theme's component parts are held to the raised
floors too, and one under them draws its family default. The window's tokens, the terminal's
colours and the parts' stylesheet all come from the one theme the engine answers with, so the
setting holds everywhere.

**Each window asks with its own layer.** The window hands the engine its person's contrast and
colours with each ask of a theme, and is handed its first frame drawn with the layer the layout
file held as it was created. A theme's specimen and the gallery's card pictures show the theme
itself and are drawn without the layer; a card drawn on the window while the pointer is on it
wears it.

### The core tokens

Lengths that should grow with the words are in rem, written as "N px at 14px"
(`calc(6rem / 14)`), so the text-size setting scales them with the window. Corners and focus
rings stay in px. The values below are Purlis Classic's, as `app/src/theme/geometry.css` writes
them, in px at the default 14px text.

**What each token reaches is said once, in the theme docs**: the "What it reaches" column of
[`themes.md`'s Geometry section](../crates/purlis-core/docs/themes.md#geometry). That table's
tokens, units, bounds and values are held to the engine's `STEPS` by `geometry_tests.rs`; its
"What it reaches" words are kept by hand, so a change to what a token reaches updates them in
the same change. They are not repeated here. `--font-ui` and `--font-mono`
are no geometry token: they are the window's own family and its one monospace family, a theme's
`fonts.ui` and `fonts.mono`.

| token                   | value                                                        |
| ----------------------- | ------------------------------------------------------------ |
| `--space-0`             | 0                                                            |
| `--space-1`             | 2px                                                          |
| `--space-2`             | 4px                                                          |
| `--space-3`             | 6px                                                          |
| `--space-4`             | 8px                                                          |
| `--space-5`             | 12px                                                         |
| `--space-6`             | 16px                                                         |
| `--space-7`             | 24px                                                         |
| `--space-8`             | 32px                                                         |
| `--font-size-2xs`       | 10px                                                         |
| `--font-size-xs`        | 11px                                                         |
| `--font-size-sm`        | 12px                                                         |
| `--font-size-md`        | 13px                                                         |
| `--font-size-base`      | 14px                                                         |
| `--font-size-lg`        | 16px                                                         |
| `--font-size-xl`        | 20px                                                         |
| `--font-weight-regular` | 400                                                          |
| `--font-weight-medium`  | 500                                                          |
| `--font-weight-strong`  | 600                                                          |
| `--line-height-tight`   | 1.2                                                          |
| `--line-height-body`    | 1.4                                                          |
| `--font-ui`             | `ui-sans-serif, Inter, Avenir, Helvetica, Arial, sans-serif` |
| `--font-mono`           | `ui-monospace, Menlo, Consolas, monospace`                   |
| `--radius-sm`           | 3px                                                          |
| `--radius-md`           | 4px                                                          |
| `--radius-control`      | 4px                                                          |
| `--radius-lg`           | 6px                                                          |
| `--radius-xl`           | 8px                                                          |
| `--radius-full`         | 9999px                                                       |
| `--size-control`        | 28px                                                         |
| `--size-chrome`         | 21px                                                         |
| `--window-gutter`       | 0px                                                          |
| `--panel-corner-radius` | 0px                                                          |
| `--panel-border-width`  | 1px                                                          |
| `--separator-width`    | 1px                                                          |
| `--focus-ring-width`    | 2px                                                          |
| `--focus-ring-offset`   | 2px                                                          |
| `--focus-ring-inset`    | -2px                                                         |
| `--pane-border-width`   | 1px; the line around a pane, none in Purlis (#2914)          |
| `--elevation-raised`    | 0 2px 8px                                                    |
| `--elevation-overlay`   | 0 8px 24px                                                   |

### What a theme may set them to

A theme names a geometry token by the name in the first column. Purlis Classic's compact is
what compact density draws for it; a theme tunes its own in `compact`. The bounds are the
engine's (`purlis_core::theme::geometry::STEPS`), which the published schema carries too.

| theme token | property | bound | Classic | Classic compact |
| --- | --- | --- | --- | --- |
| `space.0` | `--space-0` | 0 in every theme | 0 | 0 |
| `space.1` | `--space-1` | 0 to 4 px at 14px | 2 | 2 |
| `space.2` | `--space-2` | 1 to 8 px at 14px | 4 | 3 |
| `space.3` | `--space-3` | 2 to 10 px at 14px | 6 | 4 |
| `space.4` | `--space-4` | 4 to 12 px at 14px | 8 | 6 |
| `space.5` | `--space-5` | 6 to 16 px at 14px | 12 | 8 |
| `space.6` | `--space-6` | 8 to 24 px at 14px | 16 | 12 |
| `space.7` | `--space-7` | 12 to 32 px at 14px | 24 | 16 |
| `space.8` | `--space-8` | 16 to 48 px at 14px | 32 | 24 |
| `font-size.2xs` | `--font-size-2xs` | 8 to 12 px at 14px | 10 | 10 |
| `font-size.xs` | `--font-size-xs` | 9 to 13 px at 14px | 11 | 11 |
| `font-size.sm` | `--font-size-sm` | 10 to 14 px at 14px | 12 | 12 |
| `font-size.md` | `--font-size-md` | 11 to 15 px at 14px | 13 | 13 |
| `font-size.base` | `--font-size-base` | 12 to 18 px at 14px | 14 | 14 |
| `font-size.lg` | `--font-size-lg` | 13 to 22 px at 14px | 16 | 16 |
| `font-size.xl` | `--font-size-xl` | 16 to 28 px at 14px | 20 | 20 |
| `font-weight.regular` | `--font-weight-regular` | 300 to 500 | 400 | 400 |
| `font-weight.medium` | `--font-weight-medium` | 400 to 600 | 500 | 500 |
| `font-weight.strong` | `--font-weight-strong` | 500 to 800 | 600 | 600 |
| `line-height.tight` | `--line-height-tight` | 1 to 1.4 × text | 1.2 | 1.2 |
| `line-height.body` | `--line-height-body` | 1.2 to 1.8 × text | 1.4 | 1.3 |
| `radius.sm` | `--radius-sm` | 0 to 8 px | 3 | 3 |
| `radius.md` | `--radius-md` | 0 to 10 px | 4 | 4 |
| `radius.control` | `--radius-control` | 0 to 14 px | 4 | 4 |
| `radius.lg` | `--radius-lg` | 0 to 14 px | 6 | 6 |
| `radius.xl` | `--radius-xl` | 0 to 18 px | 8 | 8 |
| `radius.full` | `--radius-full` | 0 to 9999 px | 9999 | 9999 |
| `size.control` | `--size-control` | 20 to 40 px at 14px | 28 | 24 |
| `size.chrome` | `--size-chrome` | 16 to 32 px at 14px | 21 | 18 |
| `window.gutter` | `--window-gutter` | 0 to 16 px | 0 | 0 |
| `panel.corner-radius` | `--panel-corner-radius` | 0 to 24 px | 0 | 0 |
| `panel.border-width` | `--panel-border-width` | 0 to 2 px | 1 | 1 |
| `separator.width` | `--separator-width` | 0 to 2 px | 1 | 1 |
| `focus-ring.width` | `--focus-ring-width` | 1 to 4 px | 2 | 2 |
| `focus-ring.offset` | `--focus-ring-offset` | 0 to 4 px | 2 | 2 |
| `focus-ring.inset` | `--focus-ring-inset` | -4 to 0 px | -2 | -2 |
| `pane.border-width` | `--pane-border-width` | 0 to 2 px | 1 | 1 |
| `elevation.raised-y` | `--elevation-raised` | 0 to 8 px at 14px | 2 | 2 |
| `elevation.raised-blur` | `--elevation-raised` | 0 to 24 px at 14px | 8 | 8 |
| `elevation.overlay-y` | `--elevation-overlay` | 0 to 16 px at 14px | 8 | 8 |
| `elevation.overlay-blur` | `--elevation-overlay` | 0 to 48 px at 14px | 24 | 24 |

Tailwind reads these tokens and has no geometry of its own (`styles.css` hands them over with
`@theme inline reference`, #1837):

- `rounded-sm` to `rounded-xl` are the core corners;
- its spacing step (`--spacing`) is `--space-2`, so `gap-1` is 4px and `p-3` is `--space-5`;
- `text-xs` to `text-xl` (with `text-md`) are the six core type sizes, and no other `text-*`
  size exists.

The core keeps its own names (`--space-*`, `--font-size-*`), so Tailwind never redefines a token.

### Component parts

A token reaches every control drawn from it; a **component part** (ADR 0103) reaches one part of
one family, such as `button.root.corner-radius: full` for a pill. The theme engine holds the
closed manifest (`purlis_core::theme::parts`, printed by `purlis theme parts`), refuses a value
outside its bound and any size inside a state, holds the floors per part and state, and writes
one stylesheet keyed on `data-scope`, `data-part`, `data-variant` and React Aria's state
attributes, which `theme.ts` puts in the `parts` layer, above `components` and below `forced`.
A theme that names no parts writes nothing there, so the family's own stylesheet draws it.
`ui/button/button.css` is the family default the engine measures against, held to it by
`parts_tests.rs`. A size or a border width a theme sets is the family's md size and never
reaches a button at sm or lg or in a wrapping row, and a variant's rule reaches only a button of
that variant (#2904). `ui/field/field.css` is the field families' default (`field`,
`text-input`, `textarea`, #2905), held to it the same way; a family with no variants is keyed on
its part alone. `crates/purlis-core/docs/themes.md` has the grammar, the bounds and the floors.

### The control baseline

`app/src/base.css`, in the `base` layer under `App.css`, every selector `:where` so it has no
specificity:

- **every** `button`, `input`, `select` and `textarea` takes the window's font, not WebKit's
  13.33px system font;
- **one with no class** takes the whole look from the tokens: no native appearance, the field
  surface (`surface.sunken`) or the control surface (`control.base`, `control.hover` under the
  pointer), a 1px `border.control`, `--radius-control`, `--space-4` at the sides, an arrow on a
  select drawn in the text's own colour, and the outside focus ring.

Its height comes from padding, never from `height` or `min-height`: about 180 controls have no
class and are drawn by a container's rule (`.answer button`, a strip's `×` in 21px chrome), and a
minimum height would beat that rule and push a strip taller. An unclassed control no rule draws
comes out `--size-control` tall. A control with a class is its rule's; the primitive classes
(DS-2) give each kind one.

### A tab's marks, and the lines between rows

What a tab on the three strips draws, and in what (the operator's asks of 2026-10-13):

- **The title** is `--font-size-md`, one step under the window's text, on every strip. A strip's
  height is a floor (`--size-chrome` on the chat strip), not a share of the text.
- **Counts are one pill** (`CountPill.tsx`, `.count-pill`): the open chats in the neutral count
  style and, where any needs you, a second segment in the needs-you colours, at `--font-size-xs`
  with `--space-2` at the sides. One accessible name says both ("3 chats, 1 needs you"). A
  workspace's tab, the project root's and a project's draw it; a project's has the needs-you
  segment alone. On a tab too narrow for its title and its pill, the pill goes first
  (`wholeWays.fitPill`).
- **Live is a dot** (`.live-dot`): `--space-3` across, in the accent, with a ring of the same
  colour, right after the name. Named "Live". A row with room (the Explorer, the Saving tab)
  keeps the glyph.
- **A pin is a corner mark** (`.pinned`): `--space-1` in from the tab's top corner at the end,
  in the muted text colour, and still a named mark and not a control.
- **The project root's tab** is a house with a tab's padding (`--space-4`), as wide as what it
  holds; its measured width comes off the strip's room (`fits.useWidth`).
- **One hairline, `border.subtle`,** frames the workspaces' row below, as the title bar's frames
  it above, and parts the Panel's two halves. Under the workspaces it is drawn over the row and
  the selected tab is raised over it, so that tab joins its content. Between the halves it is
  their resize handle, and each half keeps `--space-2` clear of it.

### The guards, and the lists that only shrink

Four guards beside `designSet.guard.test.ts` keep the window from drifting again (#1837; the
third, `type.guard.test.ts`, is [the text roles'](#the-type-guard), #1920; the fourth holds
the focus rings, #2867):

- **`geometry.guard.test.ts`** fails on a literal **padding**, **margin**, **gap**, **corner**,
  **font size** or **offset** (`top`, `right`, `bottom`, `left`, `inset` and its forms), a
  **scroll margin or padding** (`scroll-margin*`, `scroll-padding*`, read as a margin or a
  padding) and the grid's names for a gap (`grid-gap`, `grid-row-gap`, `grid-column-gap`), in
  any stylesheet but `theme/geometry.css`; in a component's or a script's style, followed
  wherever the file hands it on ([what the guards read in script](#what-the-guards-read-in-script));
  or as a Tailwind class (`scroll-mt-*` and `scroll-p-*` among them), read
  past its variants (`md:`, `data-[open]:`, `[&>*]:`): off the core scale (`p-5`, `start-5`,
  `text-2xl`, `rounded-2xl`), with a value of its own (`p-[7px]`, `text-[13px]`,
  `rounded-[5px]`) or as an arbitrary property (`[padding:7px]`). A custom property that holds
  a length is held to what reads it, in a stylesheet or a component (an inline `style`, a
  utility such as `p-(--pad)` or `[padding:var(--pad)]`): one a padding, margin, gap, corner,
  font size or offset reads, or that nothing reads, counts, so `--pad: 7px` is no way round,
  set in a stylesheet, inline or by `setProperty`. Not literals: `0`, a `1px` hairline, `1em`,
  `1lh` or `1ch` (as big as the element's own text), a share (`%`, `vh`, `vw`), and a token, or
  a token's whole multiple or whole share (`* 3`, `* -1`, `/ 2`). A length is read in every
  unit there is: the absolute ones and those of the text, the element's own (`em`, `ex`, `ch`,
  `ic`, `cap`, `lh`) and the root's (`rem`, `rex`, `rch`, `ric`, `rcap`, `rlh`), so `2rlh` or
  `1.5cap` is a literal as `7px` is, and a number with an exponent is read for the value it
  gives (`7e0px` is `7px`; `1e0px` is a hairline). A property is read past its vendor's prefix
  (`-webkit-border-radius`, `WebkitBorderRadius`), and a value a ternary picks, branch by branch
  (`padding: on ? "7px" : 0`). The guard reads the
  arithmetic of every CSS math function (`calc()`, `min()`, `max()`, `clamp()`, `round()`,
  `mod()`, `rem()`, `abs()`, `sign()` and the rest, `-webkit-calc()` too): a number in it that is not whole is a
  literal however it is spelled (`* 3.5`, `* +0.5`, `* calc(3.5)`, `* 35e-1`). A product's
  factors fold into one net factor, which is whole or a whole share (`* 7 / 2` is a literal,
  `/ 2` is not). A term that holds a written length takes a whole share and no multiple
  (`calc(1px * 7)`, `calc((var(--space-0) + 1px) * 7)`; `(var(--size-control) - 1lh) / 2`
  passes), a sum writes each length once (`calc(1px + 1px)` is a literal), and a checked custom
  property whose value is a bare written length (`--hair: 1px`) is that length where it is
  read. The two structural reserves sized
  in controls, below, pass only in the declaration each is written in. A shadow's
  lengths are elevation, which this guard does not read. **A box's size** (a width, a height
  and their min, max, block and inline forms, or a custom property only they read) is not a step
  of the scale, so the guard does not read it either (#2319): a dialog's width, an icon's square
  and a strip's cap on a tab are each that box's own.
- **`controls.guard.test.ts`** fails on a native control that is not its family's and does not
  wear its class (`docs/ui-primitives.md`, "Every control is a family's"): a `<button>` or a
  Radix trigger without `asChild` (a list's row or a tree's row, `ui-row` or `ui-tree-row`; a
  button is the button family's `Button`, a segment the choice family's), an `<input>` or
  `<textarea>` (none: a box is the field family's `TextField` or `TextArea`, a checkbox or a
  radio the choice family's), a `<select>` (none: the select family's `Select`), a `<table>`
  (`ui-table`), and a Radix menu's content and rows (none: the menu family's). **It holds two
  heights too** (#2449): a `height`, `min-height`, `block-size` or `min-block-size` set on a
  control is `--size-control`, and one set on a strip or a pane header is `--size-chrome`. A
  control is `--size-chrome` only as the small form: a control whose selector puts it inside a
  strip. A field of several lines is a whole number of controls. A control is what the rule's
  subject names (each selector of a list on its own, a nested rule as its full selector, an
  `:is()` or `:where()` as each selector in it): a `button`, `select`, `textarea` or non-toggle
  `input`, or a class some component gives a control (a list's row is its list's). A strip or a pane header is a class named for one (`-bar`,
  `-strip`, `-head`, `-tabs`, `strip-*`, the three strips, `.pane-chips`, the status line; not
  a chart's bar or a side's column) and the rows of controls a strip or a pane's top line draws
  as part of itself (`.pane-task-ends`, `.tab-tasks`, and the top line's chips). **The pane's
  top line is one height** (#2484): every chip on it (the breadcrumb, Brief, the gauge, the
  harness, where the chat came from, its work item: `PANE_LINE_CHIPS`) is set to
  `--size-chrome`, and the guard fails a chip that is not. A strip's part is the strip's height
  even when it is a button. The guard reads what `LayoutPanes.tsx` draws in the line, so a new
  chip fails until it is listed and held. A height that is its content's or its
  container's (`auto`, `100%`) passes. Heights are held where a rule declares one: a control
  with no declared height (one its padding makes tall) is not checked, and a custom property is
  held by its value, not by the scope it is declared in. The exceptions are named with their
  reason in the guard (`HEIGHT_EXCEPTIONS`): the title bar is the system's height, the
  workspaces' row, the main navigation row, keeps control-height tabs (the operator,
  2026-10-17), an activity tab is
  the square of its column, and two icon or confirm buttons outside a strip are drawn at the
  small control height.
- **`focusRing.guard.test.ts`** fails on a focus ring that does not read the focus-ring tokens
  (#2867): in a rule for a focused element (`:focus`, `:focus-visible`, `:focus-within`,
  `[data-focus-visible]`) or in a component's or a script's style, an `outline` or
  `outline-width` is `var(--focus-ring-width)`, and an `outline-offset` reads
  `--focus-ring-offset` (outside), `--focus-ring-inset` (inside) or `--focus-ring-width`, a whole
  share or multiple of one allowed. A written length, a width keyword (`thin`) or a Tailwind
  `outline-<n>` fails; a ring taken away (`none`, `0`) passes. A ring that sits half as far out
  or in as the token says is `calc(var(--focus-ring-offset) / 2)` or
  `calc(var(--focus-ring-inset) / 2)`. The forced-colours sheet is not read: no theme token
  reaches it, so its ring is the system's.

Each started with **an allowance list of the offenders the window had when it landed**,
grouped by file and one entry per line. The geometry and type lists reached zero with #2319
and are gone, so a literal in what those two guards read fails; `controls.guard.allowance.ts`
remains. An entry names a declaration by its rule and what
it sets (`.chat | padding: 0.1rem 0.2rem`), or a control by its tag as written (`<button className="tab-close">`), never by line
number. The list only shrinks:

- a new offender fails: take the value from a token, or give the control its class;
- an entry with nothing left to name fails: delete its line when you fix it;
- a file's list longer than its frozen ceiling (`CEILING` in the controls guard) fails, and a file
  without a ceiling may have no list.

The surface tickets migrated a view and deleted its lines in the same change. Two of them
editing one file's list touched different lines, so their removals merged.

**A migration snaps to the scale** (the operator, 2026-10-17, #2313). A spacing, corner, type
size or weight with no exact token takes the nearest one, by computed px at the 14px root (a tie
goes to the smaller), and a px literal may become a rem token. Three more rules go with it:

- **An evenly stepped series takes a multiple of one token** (`calc(var(--space-5) * 3)`), so its
  steps stay even rather than flattening onto the top of the scale. That is why the geometry
  guard passes a token's whole multiple and whole share (an indent per level of a tree or a
  menu is the level's number of steps), and nothing in between: a product's factors fold into
  one net factor first, so `* 7 / 2` is the `3.5` it is.
- **A corner that draws the same is the pill.** A radius at or above half its box's height
  becomes `--radius-full`, whatever its literal was (`999px`, or `0.7rem` on a box 1.35rem tall).
- **A structural size is outside the snap**: a size that is no step of the scale, such as an
  icon's box or a strip's cap on a tab's width, keeps its literal, and the geometry guard does
  not read a box's size. A structural reserve written as a padding or a margin is a multiple of
  a size token. Two are not whole, and the guard names each with the one declaration it is
  written in (`RESERVES`): the title bar's drag stretch (`.title-bar`'s `--title-bar-drag`,
  `calc(var(--size-control) * 2.5)`) and the pane chips' room for the corner's controls (the
  `--pane-controls` fallback in `.pane-chips:has(.pane-crumbs)`'s `margin-right`,
  `calc(var(--size-control) * 2.75)`). The same expression anywhere else is a literal.
- **An offset snaps as a margin does** (#2450): a `top`, `left` or `inset-inline-end` takes the
  nearest spacing token, as the pane's corners and the activity bar's count do.
- **A line height keeps `1`**, a line as tall as its text, which the type guard reads as no
  leading. Any other line height takes the nearest line-height token by computed px, as a size
  does (#2319: a brief's text and the Welcome page's lede onto `--line-height-body`, the rail's
  count onto `--line-height-tight`).

Each snap is listed with its change in px in the pull request, and the window is looked at
before and after.

### What the guards read in script

The geometry and type guards read a component's or a script's style through one reader,
`styleEntries` in `guards.testkit.ts`, and the controls guard reads a control's `style` through
the same one (`tagStyleKeys`), so the three see a style the same way (#2498, #2566). The reader
starts where a style is handed: a `style={…}` expression, a spread in a style object, the
objects `Object.assign` writes onto an element's style, and a style object typed or cast as
`CSSProperties`. From there it follows the style wherever the file hands it on:

- an object written there (`{{ … }}`, `{ … } as CSSProperties`), and each object it spreads;
- a variable that holds it (`style={held}`), and a variable that holds that one;
- a member of a local object, by name or by a written key (`style={styles.box}`,
  `style={styles["box"]}`), or of a destructuring (`const { box } = styles; style={box}`);
- each branch of a ternary (`style={on ? held : undefined}`), each side of `||` and `??`, the
  last of `&&`, and what is in parentheses or before `as`, `satisfies` or `!`;
- what a local function returns, a declared function's every `return` (past a return type
  such as `: { padding: string }`) or an arrow's expression (`style={boxStyle(on)}`), what
  `useMemo` returns, and what a function `useCallback` holds returns when it is called;
- what a relative import gives (`import { box }`, `import * as styles` and its members, a
  default export, an object or a function), what an imported function returns, one called on a
  `* as` import among them (`s.make()`), and what that file re-exports (`export { box as panel }`,
  `export * from`). A relative import is the file TypeScript resolves it to: `./s`, `./s.ts`
  and `./s.js` are `s.ts` or `s.tsx`, and a folder is its `index`. Each imported file is read
  and parsed once in a guard's pass.

In script it reads an element's style however the file holds it: `el.style` and `el["style"]`
themselves and a name declared to hold one (`const st = el.style`, `const st = (el.style)`,
`const st = el.style as CSSStyleDeclaration`, `const { style } = el`, `const { style: s } = el`,
and a name that holds one of those). An
assignment to one of its properties (`st.padding = "7px"`, `st["padding-top"]`,
``st[`padding`]``), its CSS text set or added to (`st.cssText = "…"`, `st.cssText += "…"`), a
`style` attribute set as text, and every `setProperty(…)` call are read, each declaration in
CSS text on its own.

A value is read where it is written out, and the reader follows a value as it follows a style,
under the same bounds (#2684): each branch of a ternary and each side of `||`, `??` and `&&`; a
variable that holds it (`const pad = "7px"`, then `padding: pad`), and one that holds that one;
a parameter's default (`({ pad = "7px" }) =>`, `({ edge: pad = "7px" })`,
`function A(pad = "7px")`), for a style as for a value; a member of a local object (`sizes.pad`,
`sizes["pad"]`) or of a destructuring; the operand of a unary `-` (`margin: -pad`); what a local
function, an arrow or `useMemo` returns (`padding: padOf(on)`); and what a relative import gives,
an export under another name (`export { pad as gap }`) among them. This holds for a style
object's key, an assignment (`el.style.padding = pad`) and the value a `setProperty(…)` call
sets. Each use is read on its own, so a value held once and used twice is two findings, and a
finding says where a value not written at its use came from: `style padding: "7px" (held as
pad)`, `style margin: "2px" (returned by edgeOf())`. A name is matched across its file, not by
its block, so a parameter that shares a held value's name is read as that value: rename one of
them.

**The reader fails closed.** A style it cannot finish following fails the guard with a
sentence that names the file and the style, and is never passed over:

- a style or a value handed on more than `FOLLOW_LIMIT` (24) times, hop by hop, from where it
  is used;
- a style or a value that comes from a relative import the guard cannot open: one above `src`,
  one whose file is not found, or one in a file that is not script (`./box.css`).

What they do not read: a value worked out (`${n}px`, `n * 4`), a member of a value
(`pad.length`), or a value given after its declaration: only a `let`'s declaration is read
(`let pad = "0"; pad = "7px"` reads `"0"`); an enum member (`P.s`), an array's item (`P[0]`), a
class field (`this.pad`), a comma expression (`(0, "7px")`), a default of a method's parameter,
and a `setProperty(…)` whose property name is held (`setProperty(name, "7px")`); a member
reached by a computed key (`styles[name]`), and a method of a local object (`styles.box()`); a
wrapper other than `useMemo` and `useCallback`; a style a component is handed through its parameters, or hands on
in a prop with another name (`boxStyle={…}`) or under an object's `style` key
(`createElement("div", { style })`); an element's style handed to a function as a parameter
(`g(el.style)`), or held by a name assigned after it is declared (`let st; st = el.style`);
an import from a package or through a path alias; CSS text assigned from a variable, or a
template literal with `${…}` in it. A cycle (`const a = b; const b = a`) holds nothing to read
and is not a failure. The controls
guard reads a control's `style` attribute only, not an assignment to an element's style in
script, which names no control. The colour and motion guard (`theme/literals.test.ts`) reads
every string a script writes, wherever it is handed, so it needs no reader of its own.

## Text roles

**Every word in the window has a rank, and a rank is one class** (#1920). An audit on
2026-10-13 counted 152 font-size declarations in 25 values, 38% of them from a token; weights
written as bare numbers; two monospace stacks; the muted colour on most secondary text; and the
section label written five ways. Each surface had ranked its own words. A **text role** is a
fixed size, weight and colour from the tokens, and a surface wears it instead.

| role      | class               | for                                                       | size                     | weight  | colour           |
| --------- | ------------------- | --------------------------------------------------------- | ------------------------ | ------- | ---------------- |
| title     | `ui-text-title`     | a view's or a dialog's title                              | `--font-size-lg`, 16px   | strong  | `text.primary`   |
| heading   | `ui-text-heading`   | a section inside a view; a card's heading                 | `--font-size-base`, 14px | strong  | `text.primary`   |
| body      | `ui-text-body`      | what the surface is about: a row's name, an ask's words   | `--font-size-md`, 13px   | regular | `text.primary`   |
| secondary | `ui-text-secondary` | where, who, a help line under a field                     | `--font-size-sm`, 12px   | regular | `text.secondary` |
| meta      | `ui-text-meta`      | age, kind, counts, shortcuts: read last                   | `--font-size-xs`, 11px   | regular | `text.quiet`     |
| label     | `ui-text-label`     | a section label over a group, in capitals spaced `0.06em` | `--font-size-xs`, 11px   | strong  | `text.secondary` |
| code      | `ui-text-code`      | a command, a path, a host: `--font-mono`                  | the role it sits in      | regular | inherited        |

`ui-text-strong` is a modifier and not a role: the one strong line of a row is a body at the
strong weight (`ui-text-body ui-text-strong`). The classes are in
`app/src/primitives/text.css`; [`ui-primitives.md`](ui-primitives.md#text-roles) says how a
view wears one.

**The rules that go with the table:**

1. **Colour follows the role, never the other way.** A word is not made quieter by picking a
   quieter colour; it is given the role it has, and the role brings the colour.
2. **Muted is for meta only.** A help line or a where line is secondary. (Muted was the
   window's most used text colour, mostly on secondary-role text.)
3. **Nothing is under the words' floor.** 11px (`--font-size-xs`) is it: `0.72rem` and
   `0.65rem` become it, and a snap never lands under it. The one step below, `--font-size-2xs`
   (10px, the operator, 2026-10-17), is for a tight secondary label only: the Chats view's and the
   Inbox's scope switch (`.ui-levels-small`).
4. **One colour for a word that says something is wrong:** `danger.text`. `state.failed` is a
   state's mark, not a word's colour.
5. **Three weights.** Regular 400, medium 500 and strong 600, and no fourth.
6. **A role sets no line height.** A word keeps the line of what it sits in:
   `--line-height-body` is the window's, `--line-height-tight` a compact line's.

### What is the theme's, and what is core

|                 | what                                                                                                                                        | where                                       | who sets it                     |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------- |
| **the theme's** | the colour values: `text.primary`, `text.secondary`, `text.muted`, `danger.text`                                                            | a theme file, written by `theme.ts`         | the theme                       |
| **core**        | the roles themselves (which size, weight and colour token a role reads), and the defaults of the sizes, weights, line heights and families, which a theme sets within bounds | `theme/geometry.css`, `primitives/text.css` | purlis; a theme within bounds   |

So a theme can make the secondary text warmer or the muted text darker, and cannot make the
meta the primary colour. Since #2326 a theme folder also sets the sizes, weights and line heights
within their bounds (`font-size.lg`, `font-weight.strong`, `line-height.body`) and its fonts
([Geometry and fonts](#geometry-and-fonts)); which size, weight and colour token a role reads
stays core's.

### Where today's pixels and the table disagree

The roles landed with **no visible change** (#1920's first step), so where a role and the
window differ, the window was kept and the difference is recorded here for the migrations:

- **`--font-size-lg` and `--font-size-xl` were documented for section headings and a view's
  title and drawn by nothing.** The table puts the title on `lg` and the heading on `base`.
  Today's titles are `1rem` (14px) and its section headings `0.85rem` to `0.95rem`, so each
  steps up when its surface moves, which is a change to see in a screenshot first. `xl` has no
  role.
- **The Welcome page's `h1` is `1.5rem` (21px)**, above the scale's top step.
- **Counts and matches were weight 700** (five rules), which no token names. The geometry snap
  rule (#2313, 2026-10-17) takes a weight with no token to the nearest one, so the counts on the
  strips, the rail and the title bar's hand are `--font-weight-strong` now (#2315); the rest move
  with their surfaces. **Settings' form labels were 500** (two rules): the type scale has three
  weights, regular 400, medium 500 and strong 600, since the foundation draws 500 throughout, and
  those labels are `--font-weight-medium` (#2317).
- **A read update in the Inbox is the body's size in the secondary colour**, which is no role.
  It wears none, and its one rule (`.inbox-update[data-read] .ask-says`) is built from tokens.
- **The second monospace stack**, `ui-monospace, monospace` (a block's report and a brief's
  text), is `--font-mono` now (#2319). The window's own sans-serif stack, which has no token
  because nothing else should name it, is given once with the 14px root size in
  `theme/geometry.css`, the core. The terminal and the editor take their type from their own
  themes and are not covered.
- **A label's capitals and its `0.06em` spacing have no token**; they are written once, in
  `.ui-text-label`.

### The type guard

`app/src/type.guard.test.ts` refuses a `font-size`, `font-weight`, `line-height`,
`font-family` or text `color` that is not its token (or `inherit`, or `currentColor` for a
colour), and a `font` shorthand that is not `inherit`, in any stylesheet but
`theme/geometry.css`, in a component's or a script's style as the geometry guard reads it (an
inline `style`, a style object held in a variable, an assignment such as
`el.style.fontSize = "13px"`, CSS text, `Object.assign`, a `setProperty` call, each branch of
a ternary, a vendor's prefix read past), and as a Tailwind
utility, read past its variants as the declarations it writes: a weight by name
(`font-semibold`), a family of Tailwind's own (`font-mono`), a line height other than
`leading-none` (and a size's `/7` or `/[1.3]`, as in `text-sm/7`), a value of its own
(`text-[13px]`, `font-[600]`, `leading-[1.3]`) and an arbitrary property (`[font-weight:600]`),
unless it is the property's token. A token of another kind does not
pass: `color: var(--font-size-sm)` is a literal. Its failure names the role to wear instead,
by the size or weight that was written.

Its allowance landed at 159 lines in `App.css` (91 of them font sizes) and reached zero with
#2319, and the list is gone: [as with the geometry guard](#the-guards-and-the-lists-that-only-shrink),
a literal in what it reads fails. A line height of exactly `1` is no leading
and passes. A font size is
held by the geometry guard too.

**A token is not yet a role.** `font-size: var(--font-size-sm)` passes the guard and is still
a surface ranking its own words. The guard stops the scale growing; the migrations give the
words their roles.

### The order the surfaces move in

One change each, each checked against a screenshot (#1920):

1. the Chats list (moved: [below](#the-chats-list-on-the-roles));
2. the Inbox (its rows already wear the roles, #1919 and this change; its headings and its
   remaining lines follow);
3. the Explorer, with the Panel's Repos and Memory halves (moved:
   [below](#the-explorer-and-the-panels-halves-on-the-roles));
4. the tab strips and the status bar;
5. Settings;
6. Notices and dialogs;
7. Welcome.

Along the way the section label gets one spelling (`ui-text-label`, which is `.sidebar-title`'s
type today), and the two largest size tokens are each read by a role or removed.

### The Explorer and the Panel's halves, on the roles

The Explorer set 12px on its root and a panel 11.9px (`0.85rem`), and both told their words
apart by opacity (60% for a sentence) and by three greys. Now (#1920):

| word                                                                           | role                            | before                               | after                         |
| ------------------------------------------------------------------------------ | ------------------------------- | ------------------------------------ | ----------------------------- |
| a section's title: "Files", and "Repos" and "Memory" on the Panel's halves     | `ui-text-label`                 | 11px, strong, capitals; "Files" in the primary colour, the halves' secondary | 11px, strong, secondary, capitals |
| what Files is of, beside its title (the workspace's name)                      | `ui-text-secondary`             | 11px, regular, muted                 | 12px, regular, secondary      |
| a file's, a folder's, a repo's and a branch's name                             | `ui-text-body`, from the root   | 12px, primary (a repo's strong)      | 13px, primary (a repo's strong) |
| "the workspace itself"; why a file does not open                               | `ui-text-secondary`             | 11px, primary at 60%                 | 12px, secondary               |
| "in <repo>" after a branch                                                     | `ui-text-secondary`             | 12px, muted                          | 12px, secondary               |
| how many branches a repo has                                                   | `ui-text-meta ui-text-strong`   | 11px, regular, secondary             | 11px, strong, muted           |
| a change's letter or count                                                     | none: tokens, in its change's colour | 11px, strong                    | the same                      |
| a sentence: "Nothing here", "No branches cut here", "Asking git…", an empty view's | `ui-text-secondary`         | 12px (11.9px in a panel), primary at 60% | 12px, secondary, undimmed |
| a path purlis names and does not read                                          | `ui-text-secondary ui-text-code` | 12px, the window's face             | 12px, `--font-mono`           |
| a panel row's words; the default persona's                                     | `ui-text-body`; `ui-text-strong` | 11.9px, primary; strong             | 13px, primary; strong         |
| a panel row's note; "· 12 left"                                                | `ui-text-meta`                  | 11.9px muted; 10.5px at 60%          | 11px, muted                   |
| whose panel it is, beside an extension's title                                 | `ui-text-secondary`             | 11px, regular, muted                 | 12px, regular, secondary      |
| a branch's cockpit: its crumbs, its name, its facts                            | meta; body strong; meta         | 11px muted; 13px strong; 11px muted  | the same                      |

What it settled, beside what the Chats list did:

- **A name that a state tints takes its role from the root, not from a class of its own.** A
  changed file's name is drawn in its change's colour and a file purlis will not open in a
  quieter one. A role outranks a view's rule for the word that wears it, so the Explorer and
  a panel wear `ui-text-body` on their root and a row's name inherits it; every other word
  wears its own class.
- **A count is meta at the strong weight** (`ui-text-meta ui-text-strong`): a repo's branches.
- **A change's mark wears no role**: its colour is the change's (`--change-colour`), which a
  role would overrule. It is a meta's size at the strong weight, from the tokens.
- **A section's title is the label on the element that holds its words.** Where the title is
  a disclosure (the Explorer's sections) that is the button, which is a row of the set and
  would otherwise draw the title in a row's colour.
- **A sentence is quiet by its role, not by opacity**: `.none`, `.pending` and `.empty` are no
  longer dimmed in the Explorer or in a panel.

A panel's root is shared by every view of the Attention region and the Panel, so what sets no
size of its own there (Todos, Sessions, Personas, Vaults, a refusal) follows it from 11.9px to
13px. Not moved with it, and still on their own rules: the day a dated list is drawn under
(`.panel-day`), the `unwired` and `stale` chips (`.label`), a refusal's colour
(`state.failed` in the Explorer and in a panel, where the rule above wants `danger.text`),
and `.sidebar-title`'s own type, which goes when the Inbox's headings wear the label too.
After this change the type guard's `App.css` list is 13 lines shorter and the geometry
guard's 2.

### The Chats list, on the roles

The first surface to move (#1920). The list set `0.85rem` (11.9px) on its root, so a name, a
note and a heading were one size told apart by colour, with four more literal sizes on its
notes, two of them under 11px. Now:

| word                                                                     | role                           | before                                  | after                          |
| ------------------------------------------------------------------------ | ------------------------------ | --------------------------------------- | ------------------------------ |
| the title, "Chats"                                                       | `ui-text-label`                | 11px, strong, secondary, capitals       | the same                       |
| a chat's name                                                            | `ui-text-body`                 | 11.9px, regular, primary                | 13px, regular, primary         |
| a line under a row (task limit, waiting on memory, a restart's answer)   | `ui-text-secondary`            | 10.1px, secondary                       | 12px, secondary                |
| the notes beside the title ("The filter hides 3 of 12 chats.")           | `ui-text-secondary`            | 10.7px, muted                           | 12px, secondary                |
| "No chats are running…"                                                  | `ui-text-secondary`            | 11.9px, primary at 60% opacity          | 12px, secondary, undimmed      |
| a finished task's name, "Finished (n)", a finished task's note           | `ui-text-secondary`            | 11.9px, primary (the note muted)        | 12px, secondary                |
| a finished task's report; where it was written                           | `ui-text-body`; `-secondary`   | 11.9px, primary; 11px, muted            | 13px, primary; 12px, secondary |
| a folded row's task count; "Stopping…"                                   | `ui-text-meta` (the count strong) | 10.5px and 10.7px, regular, muted    | 11px, muted; the count 600     |
| a workspace's heading over its sessions (the list grouped)               | `ui-text-label`                | 10.1px, regular, muted, spaced `0.04em` | 11px, strong, secondary, `0.06em` |
| a row's card: its name; its sentences and values; a fact's word          | body strong; secondary; meta   | 13px strong; 13px primary; 13px muted   | 13px strong; 12px secondary; 11px muted |

What it settled, for the surfaces that follow:

- **A surface whose words are mostly one role wears that role on its root** (`ui-text-body` on
  the section, `ui-text-secondary` on the card), so a mark sized by the text beside it (`em`)
  and a word no component ranks take that role's size, and each ranked word wears its own.
- **No row is strong.** A chat that needs you says so by its mark, as it did; the list had no
  heavier name and was given none.
- **A count is meta at the strong weight** (`ui-text-meta ui-text-strong`), as the Explorer's
  are: a folded row's task count, which was regular.
- **A row's height was not touched**: a chat's row was already the shared tree row
  (`--ui-row-height`, 21px at the default text, a file's row's height in the Explorer), and
  `workspace-explorer.e2e.ts` now measures the two against each other. What stands under or
  beside a row (a line under it, the folded count, a workspace's heading) is spaced from the
  geometry scale, and a line under a row starts where the row's name does.

Not moved with it, and still on their own rules: the filter's chips and the scope switch
(controls), the *Go* button beside the title, and the second step of ending a task
(`.task-end-confirm`), which the pane's breadcrumb shares.

## Menus

**A menu is short verbs in a few groups, and the same in every place** (the operator's report
of 2026-10-13: every row of a chat tab's menu repeated the tab's name, almost every row had a
sentence or two under it, a row per persona ran the menu off the window, and five rows at its
end were red). [`ui-primitives.md`](ui-primitives.md#menus) has the seven rules and where each
is applied; what they draw with is this:

Every menu is the menu family (#2909), drawn by its parts (`ui/menu/menu.css`) on the popover
family's surface (`ui/popover/popover.css`):

| part                     | attributes                                   | role, size and colour                                              |
| ------------------------ | -------------------------------------------- | ------------------------------------------------------------------ |
| a row                    | `menu` `root`, variant `default`             | `--size-control` tall at least; `--space-3` inside, `--space-1` between rows; `tab.active` under the pointer or the keyboard, ringed inside from the keyboard |
| its words                | `menu` `label`                               | body: 13px, the row's colour; one line, cut with an ellipsis       |
| what is under them       | `menu` `description`                         | secondary: 12px, `text.secondary`; wraps, never cut                |
| its tooltip              | `menu` `tip`, `ui-tooltip ui-text-secondary` | secondary, on the overlay surface, at most nine of `--space-8` wide |
| its shortcut             | `menu` `shortcut`                            | meta: 11px, `text.quiet`, at the row's end                         |
| a tick or a submenu's arrow | `menu` `indicator`                        | `text.muted`, at the row's end                                     |
| a group's name           | `popover` `header`                           | label: 11px strong capitals, `text.secondary`; purlis's own words only |
| a group named by data    | `popover` `header` `data-named`              | secondary, as the name is written: no capitals (T4)                |
| the line between groups  | `popover` `rule`                             | 1px of `border.subtle`                                             |
| what cannot be undone    | `menu` `root`, variant `danger`              | `danger.base` words on `danger.surface` under the pointer          |
| a row that cannot run    | `menu` `root` `data-disabled`                | its words in `text.faint`; the line saying why keeps its tier      |
| the menu                 | `popover` `root`, variant `menu`             | the overlay surface; `calc(var(--space-8) * 6)` to `* 10` wide (192px to 320px), placed under its trigger at least 8px from the window's edge |

The rules in one line each:

1. A row is a short verb and does not repeat the name of the thing it was opened on.
2. A note is a tooltip and a description. Words stay under a row only for what cannot be
   undone and for why a row cannot run, and are never cut: they wrap.
3. Rows of one kind are one row with a submenu; at most ten rows in four groups at the top.
4. The danger colour is only for what cannot be undone.
5. An ellipsis only where the row asks something first; no greyed row that only says what a
   press would do.
6. A shortcut at the row's end; a toggle is one row with a tick.
7. Type and geometry from the tokens above, and no literal.

**No token names a menu's width.** The least and the most are written from the spacing scale
(`calc(var(--space-8) * 6)` and `* 10`), as a settings row's widths are, and a surface that
needs another hands the menu `--menu-min` or `--menu-max`. A row's tooltip is over its menu,
one layer above React Aria's own for the surface; neither layer has a token.

## Tailwind, shadcn and Lucide

**Tailwind v4**, wired in `app/src/styles.css`. Three decisions there, each with a test in
`app/src/theme/tailwind.test.ts` that fails if it is undone:

- **Tailwind owns no colour.** Every entry in `@theme` is `var(--<token>)`. v4's `@theme` is
  CSS-variable-native, which is why v4 and not v3: it sits on top of the token layer instead of
  being one.
- **Tailwind's palette is deleted.** `--color-*: initial` clears the namespace, so `bg-red-500`
  and `text-slate-300` are not classes — they are typos. That is the "semantic, not palette"
  rule enforced by the build rather than by whoever reviews the diff. `transparent` and
  `current` are put back, because they are the absence of a colour and the inherited one.
  Checked, because deleting a namespace is the sort of thing that takes a neighbour with it:
  `ring-2` still compiles and now falls back to `currentcolor`, which is the token-driven text
  colour and is better than the blue it used to default to. **A shadow is a token's colour
  too** (M6.8): Tailwind's `shadow-md` and its siblings carried their own `rgb(0 0 0 / 0.1)`,
  so every shadow namespace — `shadow`, `inset-shadow`, `drop-shadow`, `text-shadow` — is
  cleared like the palette and its sizes are put back drawn in `overlay.shadow`.
  `tailwind.test.ts` compiles every one of them to hold that.
- **Motion is bridged the same way.** `ease-*` and `animate-*` are cleared like the palette, so
  `ease-in-out` and `animate-spin` are not classes; `ease-enter` and `duration-enter` are the
  motion tokens; and a bare `transition` takes `duration.quick` and `easing.standard` instead of
  Tailwind's 150 ms. Tailwind still makes `duration-150` from any integer and no `@theme` entry
  can stop it, so `literals.test.ts` refuses one in the source.
- **Preflight is not imported, and `App.css` is imported into a layer.** A reset would restyle
  1,330 lines in one commit, and `scenario tests` read the real DOM. The layer order —
  `theme, base, vendor, charter, components, utilities` — is what the reset would have been for:
  unlayered CSS beats layered CSS, so `App.css` had to go _into_ a layer or no utility could
  ever override it. Turning preflight on is its own change, with its own evidence.

**The Tailwind colour name is the token name**, stutter and all: `text-text-primary`,
`border-border-subtle`. A prettier alias would be a second vocabulary.

> **A dependency's stylesheet is in a layer too — `vendor`, below `charter`** (M6.8). xterm's
> own stylesheet used to be imported from `SessionPane.tsx`, which put it **outside every
> layer**, and unlayered CSS beats every layer at any specificity. One declaration of it did
> collide, and it was measured (purlis#193): `.xterm .xterm-viewport { background-color:
#000 }`. A terminal is whole rows in a box that is not, so every pane has up to a row of slack
> at its bottom, and that strip was pure black under a terminal drawn in `#181818` — the
> operator's _"harness bottom seems overflowed - you can see black space"_. It was patched with
> an inline style read from the theme once per pane, which a theme switched later could not
> reach. Now `styles.css` imports it as `@import "@xterm/xterm/css/xterm.css" layer(vendor);`,
> `App.css` paints the strip `var(--terminal-background)` and wins by layer order, and a live
> theme switch repaints it with everything else. `pane-fill.e2e.ts` holds the pixel.
>
> **The guard is structural**, because `literals.test.ts` reads purlis's own sources and a
> colour a dependency ships is invisible to it: the same test refuses a stylesheet imported from
> anywhere but `styles.css`, and an `@import` there without a `layer(...)`.

**shadcn/ui: the conventions, and components when something needs one.** `cn` is at
`app/src/lib/utils.ts`, at shadcn's address with shadcn's two dependencies, so a component
pasted from shadcn finds what it expects. Nothing else is here yet, deliberately: copying in
components nothing renders is dead code, and `class-variance-authority` arrives with the first
component that has variants.

> **Settled by the operator on 2026-09-22 — copy shadcn components in.** This file used to flag
> a conflict here: `docs/ui-primitives.md` said _"Do not write a wrapper layer around them. No
> `<Modal>`, no `<Field>`, no house component library,"_ and a shadcn component is literally a
> thin wrapper around a Radix primitive. The ruling is that those are two different things
> wearing one word. A component library is **a dependency that owns your markup** — an upstream
> you cannot edit, an API you are stuck with, a look you fight. **A copied-in component is our
> own code in our own repo, editable line by line.** The rule is now _no library between you and
> the primitive, and no indirection you cannot read_, and copied-in source is neither. purlis
> **ADR 0037**'s amendment of 2026-09-22 holds the decision and the reasoning, and is
> authoritative over both this file and `ui-primitives.md`.

**What a copied component has to satisfy.** This file rather than `ui-primitives.md` has the say
on the first three, because they are about what the window is drawn _in_:

- **Every Tailwind colour class has to be renamed to this vocabulary, by hand, and nothing will
  tell you if you forget.** A shadcn component ships `bg-background`, `text-foreground`,
  `bg-destructive` — names from shadcn's own token set, which this app does not have. They are
  not classes here: the palette is deleted above and these were never in it, so each one emits
  **no CSS at all**. Nothing goes red. `literals.test.ts` catches a hex literal and an arbitrary
  value; it does not catch a class that does not exist, and `tailwind.test.ts` checks that the
  palette is gone rather than that a source file avoided it. The result of missing one is an
  element rendered undressed — which is exactly the `claudeclaudeclaudebuilt-indefault` defect
  ADR 0037 was written about. **Read the copied file's classes against the `@theme` block
  in `app/src/styles.css` before the PR, and look at the component running.**
  `bg-surface-base text-text-primary` is the shape they should end up in.
- **No arbitrary value survives the paste** — `text-[13px]`, `rounded-[6px]`, `bg-[#fff]`. This
  one _is_ mechanical: `literals.test.ts` reads the real source tree and fails on it.
- **No colour literal**, in the same test, for the same reason as everything else in this file.
- **It goes at shadcn's address**, `app/src/components/ui/`, and it is **edited freely**. That is
  the condition rather than a permission: a copy kept pristine "because upstream will fix it" is
  a dependency with worse ergonomics and no version, and there is no upstream once it is copied.
- **Say where it came from, and at what version, in the file.** No manifest records a copied
  file, so the file is the only place its provenance can live.

Still refused, unchanged by the ruling: a component library as a **dependency**, and a house
abstraction layer over Radix — `<ConfirmModal open onConfirm>` — whether it is written here or
copied from somewhere. Copying it would not launder it; what is refused is a purlis API in
front of the primitive. **The one exception** is the settings set (ADR 0037, amended
2026-10-04, V89f and V89j): SettingsLayout, SettingGroup, SettingRow, Field, Choice and
SettingActions in `app/src/settings/components.tsx`, six thin pieces over the Radix primitives
and native elements already in use, drawn in tokens. `docs/ui-primitives.md` gives the reasons. Nothing else joins it without a new
amendment.

**A dialog's footer** (the dialog family, `app/src/ui/dialog/Dialog.tsx`, #2910, which replaced
the answer bar of #1210) is the one row every dialog is answered from, a question's and a form's
alike: the quit warning, a delete's confirm, approving a project, Restart to update, About, New
workspace, Rename, Start a chat. Its answers are the dialog's props, so the rules are kept by
construction:

- **At the trailing edge, always in view.** The footer is the dialog's last row, under its rule,
  and the body above it is what scrolls, so every dialog's answers stand where the reader's eye
  ends it.
- **The way out first, then at most one other answer, and the main action last**, at the edge,
  in the same place in every dialog: Cancel then Delete, Close then Check again then Install,
  Cancel then Close then Smart close, Cancel then Create workspace. **One rule for forms and
  questions** (N2): a form's act no longer leads its row, as `SettingActions` had it (V89j); it is
  the footer's main action, trailing.
- **What cannot be taken back says so** with `danger` (which acts those are: `ui-primitives.md`,
  D-1719-3), and it is never the one Return finds from a field: an alert dialog starts on its way
  out (or, for Reopen your sessions, the answer that loses nothing). A confirm by typed name keeps
  its exception (D-1210-9): its keyboard lands in the name box, and Return there deletes only
  once the name is typed exactly, while the delete is enabled.
- **⏎ is the main action and Esc closes.** Return in a field answers with the main action;
  Start a chat's is pressed, never found by Return, since a start runs a command. Escape is the
  dialog's way out, or whatever that dialog says it is.

`dialogAnswers.guard.test.ts` fails on a dialog that builds an answer row by hand, and on a
dialog that puts a `danger` act in a form's `SettingActions` (D-1210-8).

**Lucide** is the icon set (`lucide-react`). The property that matters is that it draws with
`stroke="currentColor"` and `fill="none"`, so an icon takes the colour of the text it sits in
and a theme reaches it without an icon ever naming a colour. `app/src/lib/icons.test.tsx` pins
that.

**The icon layer is one CSS rule and no component.** Lucide puts `lucide` on every `<svg>` it
draws, and `App.css` sizes that class at `1em` — so an icon is the size of the text it sits in,
and no call site passes a `size`. Lucide also adds `aria-hidden="true"` to any icon given no
accessible name of its own, which is what keeps a button's name its words: `New tab` with a `+`
beside it is still `New tab` to a screen reader and to `pressOnly("New tab")`. Both facts are
pinned in `icons.test.tsx`, because the whole window leans on them and neither is ours.

**Every control has a name, and a test holds it** (DS-6, #629). `a11y.testkit.ts`'s
`expectEveryControlNamed` fails on any button, link, text box, checkbox, radio, switch, tab,
combobox, option, slider or menu item in the accessibility tree whose accessible name is empty.
`settings/Names.test.tsx` runs it on every group of every Settings level, and
`editor/Names.test.tsx` on every editor tab. A nameless control found in a file that cannot be
mended yet goes on that test's named debt list, and a debt no surface draws any more fails, so the
list only shrinks. CodeMirror draws a file's text as a text box, so the light editor names it
for the file, and each side of a comparison for its side. It is the rule axe's `button-name`,
`link-name`, `label` and `aria-input-field-name` check, without axe as a dependency.

The rules an icon has to meet here:

- **Beside words, never instead of them.** An icon-only control is one an operator has to learn.
  The exception is a control whose accessible name is already carried by `aria-label` — a tab's
  `×`, whose name is the catalogue's `End chat 3 steward`.
- **An icon that does not help a reader find something is noise at fifty sessions.** Marks go
  where they tell two kinds of thing apart (a worktree leaf from a chat leaf) or where they are
  the state (a pipeline's tick, cross or spinner). Not on every row because rows can have one.
- **Chosen by what a thing IS, not where it is.** The layout is data (`regions.ts`), so a region
  toggle drawn as "left panel" would point at the wrong edge the first time the region moved.
- **An icon that moves says _still happening_, and nothing else loops.** `.spinning` is a running
  pipeline and a listing purlis is still waiting for; `.breathing` is a queued pipeline. A
  settled answer is still, apart from the one `settle` it gets if it arrived while on screen.
  Reduced motion stops both loops and leaves the mark ([Motion](#motion-is-data-too)).
- **A contrast floor applies to an icon's colour as it does to text** — 3:1 for a graphic. A
  state colour that is too weak for words (`needs-you.base` measures 3.61:1 on `surface.base` in
  purlis-dark) may colour the mark beside the words and never the words.
- **An icon is Lucide's or the icon theme's, never a glyph.** A ✓, a ▸ or a ✗ typed as text is
  the font's: its weight, its size and its baseline, which change with the font and match no
  icon beside it. `app/src/lib/strayIcons.test.ts` fails on a glyph drawn alone as an icon, on an
  `<svg>` drawn by hand outside the files it names, and on any other icon package (DS-4, #627).
  A glyph inside words is prose, and is left alone.

**Files and folders are the one exception: they are drawn from an icon theme** (FM-3, #1106), a
data file beside the colour theme (`app/src/theme/icons.ts`). It maps a file's whole name, then
its extensions longest first, and a folder's name, to a symbol of path outlines, and each outline
names one of the `icon.*` tokens — so the icons follow light and dark with the colour theme, a
colour theme recolours every icon set, and the literal guard covers them. The hue is the point
there: it is what tells a Rust file from a TypeScript one in a tree of hundreds. purlis's own
set, `purlis-icons` (a pick of its pre-rename id is read as it, #1854), is a 67-symbol
subset of Material Icon Theme (MIT), vendored with its
licence under `app/icons/material-icon-theme/` and shown in About; `icons.vendor.test.ts` holds
the converted symbols to those files. A project picks another with `[theme] icons`, and an
extension contributes one as `contributes.icon_themes`. A symbol is drawn by `FileIcon.tsx` as React
elements it builds, never as SVG text, so a contributed icon theme has nowhere to put a script, a
link or a style.

## The interrupt budget

**At most three prompts stand between a new machine and the first answered agent turn** (W10,
DS-9). It is part of ST9's definition of done, and the first run's scenario tests hold it: every
scenario in `app/src/FirstRun.test.tsx` counts the prompts it shows and fails above three,
naming each one. The longest way to a chat those scenarios take spends all three today: the
forge question when the repo's remote does not say, the trust question on a new machine, and the
picker when there is a choice. A new prompt on that way fails the build until another one goes.

A prompt is anything that stops the operator until they answer it, read from what the page says
it is (`app/src/interruptBudget.ts`):

- every `dialog` and `alertdialog`, an open native `<dialog>` included;
- every inline question: a `group` or `radiogroup` named by its question, ending in "?", and a
  `<fieldset>` whose `<legend>` is one. Moving an ask out of a dialog onto the page does not
  make it free.

It cannot see a question named without its "?", a toast or status line that carries an action,
or a native OS dialog such as the folder picker; a review has to.

So a surface that has to ask something on the way to the first chat is named by its question,
and the budget sees it. What costs nothing is a tab opened beside the chat (the first task, the
repo's agent instructions, the harness setup), a `status` line, and a sign-in run in a shell tab
the operator can leave.

**Amended 2026-10-12: creating a project asks where it lives (#1810, Q1/Q2).** The operator
ruled in the onboarding grill: *"The welcome screen offers: Create a project (with a stack
template, FR-17) / Get a project from your forge (clone) / Open a folder on this Mac"*, and
*"Create asks where the project lives: GitHub / GitLab / only this Mac."* This amends the first
run's old rule from W10 and FR-4 (#603), *"never ask where the plane goes"*. The rule held in
`crates/purlis-core/src/firstrun.rs` and `app/src/FirstRun.tsx`.
- **Create** now asks where the project lives, and **only this Mac** is one click away. It needs
  no account, and it makes the local project as before.
- **Open a folder on this Mac** asks nothing new.
- **Forge sign-in is inline and can be skipped.** It is asked only when the chosen place needs
  it (Q2).
- The question is a prompt like any other, so this budget counts it. The welcome flow's
  scenarios keep the way to the first chat at three prompts or fewer.

### Nothing purlis says covers what a chat wrote

**A pane's Notices take a row of the pane, above its terminal, and never draw over it**
(#1647, #3142). The terminal is what the person reads to answer a question, so a question drawn over
it hides what it asks about: the operator's screenshot of 2026-10-10 had three Notices stacked
in a pane's corner over the conversation. Two stand in the row and the rest wait behind "+N
more", the band's rule (V91i), so purlis's own questions cost a chat at most two Notices' height
of terminal. Since #3142 nothing is over the terminal at all: the band and the chat at a glance
(the pane's header) each take a row inside the pane, and the terminal is pinned to its bottom.
The shape and the rules are in `docs/ui-primitives.md` ("The Notice is a house piece
too").

## What it costs

Measured on this branch, against `origin/main`:

|           | main                     | this                     | delta                  |
| --------- | ------------------------ | ------------------------ | ---------------------- |
| JS bundle | 749.64 kB (214.12 kB gz) | 755.44 kB (215.85 kB gz) | +5.80 kB (+1.73 kB gz) |
| CSS       | 18.08 kB (4.24 kB gz)    | 23.52 kB (5.23 kB gz)    | +5.44 kB (+0.99 kB gz) |

Almost all of the CSS growth is `var(--control-base)` being fifteen characters longer than
`#222`, repeated fifty-odd times; gzip takes most of it back. Tailwind's own contribution to the
built stylesheet is **995 bytes**, because it emits only what a utility uses and no utility is
used yet.

**Cold start is untouched, by construction.** ADR 0026 gives it 2 s. Nothing is read from disk
on the way to the first frame: the built-in themes are compiled into the bundle, and `main.tsx`
calls `drawIn(DEFAULT_THEME)` before `createRoot`, so no frame is ever painted in one theme and
repainted in another. That call is fifty-four `setProperty` calls on one element and measures
**0.60 ms median, 0.85 ms p95** under jsdom — 0.03% of the budget, and jsdom's CSSOM is slower
than a real engine's, so it is an upper bound. Parsing a theme file measures 0.0035 ms.

When the user theme lands it must stay off that path: apply the built-in synchronously, read the
file after, repaint if it differs.
