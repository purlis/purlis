# The foundation record

**The designer's foundation for Purlis, read once from Figma and written down here.** Work on
the Purlis theme builds from this record and does not call Figma again (#2313, "The two
shipped themes"). Purlis Classic is not described here: it is today's look, and
its theme folder, `crates/purlis-core/src/theme/built-in/purlis-classic/`, is its source.

| what          | value                                                                   |
| ------------- | ----------------------------------------------------------------------- |
| Figma file    | `A5tZlp0AjLs1VeFN5TVgXW` ("Purlis")                                     |
| page          | `0:1`                                                                   |
| frame         | `6:2` "Purlis", 1440 × 904, one dark appearance                         |
| read on       | 2026-10-16                                                              |
| frame link    | [Purlis, node 6-2](https://www.figma.com/design/A5tZlp0AjLs1VeFN5TVgXW/Purlis?node-id=6-2), for those with access to the file |
| variables     | none: the file has no variables and no styles, so every value is raw    |

The frame draws one window: a title bar, the workspace strip, a rail down each side, the Chats
panel, a file open in the editor, the Notices column, Repos and Todos, and the status bar.
Its layer names are generic (`Container`, `Text`), so this record names each value by the
region and the node id it was read from.

**Every token mapping below is a proposal.** The token names are the 79 in `TOKENS`
(`app/src/theme/theme.ts`). Where the frame draws a role purlis has no token for, the row says
**no token** and names the nearest one. Choosing the final mapping is the Purlis theme's work,
not this record's.

## Layout

The regions, top to bottom, at the frame's 1× size.

| region           | node    | size             | how it sits                                                                    |
| ---------------- | ------- | ---------------- | ------------------------------------------------------------------------------ |
| frame            | `6:2`   | 1440 × 904       | `#121215`; the window (`6:6`, 1440 × 900, radius 12) sits on it, top-aligned   |
| title bar        | `6:7`   | 1440 × 44        | padding 0 14, gap 16; project name left, three pills right (gap 6)             |
| workspace strip  | `6:23`  | 1440 × 40        | padding 0 8 6 56, gap 4; one workspace tab (height 29) and a 28 × 28 `+`       |
| body             | `6:37`  | 1440 × 786       | padding 0 8 8, gap 8: left rail, main column, right rail                       |
| left rail        | `6:7814`| 40 × 778         | padding 4 0 2, gap 6; 36 × 36 buttons, a spacer pushes two to the bottom       |
| main column      | `6:56`  | 1328 × 778       | gap 8: upper row 520 high, lower row 250 high                                  |
| Chats panel      | `6:58`  | 250 × 520        | padding 14, gap 12                                                             |
| editor           | `6:81`  | 772 × 520        | file tab strip 36.5, toolbar 46, code 437.5                                    |
| Notices column   | `6:501` | 290 × 520        | padding 14, gap 10; cards padding 12, gap 10                                   |
| Repos            | `6:532` | 660 × 250        | padding 14, gap 8                                                              |
| Todos            | `6:550` | 660 × 250        | padding 14, gap 8; rows 15.25 high, gap 8                                      |
| right rail       | `6:7881`| 40 × 778         | as the left rail                                                               |
| status bar       | `6:609` | 1440 × 30        | padding 0 16, gap 14; a spacer splits left items from right items              |

In the editor: a file tab is 30.5 high, padding 7 12, gap 8, top corners 8, tabs 2 apart, strip
padding 6 6 0. The toolbar is padding 10 14, gap 10. A code line is 21 high; the line-number
gutter is 52 wide with 18 on its right, then 30 before the first indent level and 15 for each
level after.

## Colours

Every distinct colour in the frame. "uses" counts the nodes that paint it. Each token is proposed
for one colour only; a second colour in the same role says **no token** and names the nearest.

### Surfaces

| colour    | uses | where it is used                                                                                            | proposed token (proposal)                                                    |
| --------- | ---- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `#121215` | 1    | behind the window: the frame `6:2`; shows as the 4 px under the window's rounded bottom                     | **no token**: the native window background; nearest `surface.sunken`         |
| `#141417` | 1    | status bar `6:609`                                                                                          | `surface.deep`                                                               |
| `#17171b` | 4    | the window `6:6` (title bar, workspace strip and rails draw on it); text fields `6:70`, `6:556`; the Chats scope switch's track `6:63` | `surface.base`; the fields and the track: `surface.sunken`, at the same value |
| `#1d1d22` | 5    | the five panels: Chats `6:58`, editor `6:81`, Notices `6:501`, Repos `6:532`, Todos `6:550`                  | `surface.raised`                                                             |
| `#202026` | 3    | the editor's toolbar `6:112` and code `6:130`; the file tab in front `6:104`                                 | code: `terminal.background` (no editor surface token); the tab in front: `tab.active` |
| `#24242a` | 6    | the workspace tab `6:24`; filter pills `6:74`, `6:76`; notice cards `6:507`, `6:516`; Repos count `6:546`    | workspace tab: `layer.selected`; pills: `control.base`; cards: `surface.overlay`; count: **no token**, nearest `control.count` |
| `#2a2a31` | 3    | workspace count `6:29`; the scope switch's chosen segment `6:64`; the toolbar's plain button `6:128`         | count: `control.count`; segment and button: `control.hover`                  |
| `#2f2f37` | 2    | the "Got it" buttons on notice cards `6:512`, `6:520`                                                        | **no token** for a control on a card; nearest `control.hover`               |

### Text

The frame uses six greys for text. Purlis draws them in four text tiers and faint, two of
them adjusted (below).

| colour    | uses | where it is used                                                                                       | token (#2902; the frame's proposals were three text tokens)                            |
| --------- | ---- | ------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| `#e6e4ec` | 12   | panel titles, the workspace name, the file tab in front, the file name, chosen segment, buttons        | `text.primary`                                                                         |
| `#c9c7d2` | 11   | body text: todo rows, notice sentences, the repo row, status bar workspace                             | `text.secondary`                                                                       |
| `#a09eab` | 9    | chrome labels: title bar, pills, toolbar buttons, the `+` buttons; the rails' idle icons               | `text.muted`                                                                           |
| `#8a8896` | 18   | quieter labels: tabs behind, scope segments, counts, empty states, status bar items, a notice's detail | `text.quiet`, lifted to `#91909d`; labels that are not informational stay `text.muted` |
| `#6f6d7a` | 14   | placeholders, the × on tabs, heading counts ("Notices 3"), paths, "Earlier", the status bar's folder   | words: `text.quiet` (`#91909d`); the ×: `text.faint` (`#74727f`)                       |
| `#55535f` | 32   | line numbers in the editor (and the 6 todo checkbox rings, below)                                      | line numbers: `text.quiet`; checkbox rings: `text.faint`                               |

### The adjusted greys, for the Figma file

**Two of the frame's greys are adjusted in Purlis, and the Figma file is to take the same two
values** (ruling #2803, #2902), so the next read of the frame matches the theme without a diff.
On the frame's own grounds the quieter greys miss WCAG's floors: `#8a8896` measures 4.10 on the
row under the pointer and 4.44 on a card, under 4.5; `#6f6d7a` measures 2.81 on the row under
the pointer, under 3; `#55535f` is under 3 everywhere. Two quiet text tiers cannot both pass on
these grounds, so the frame's three quietest greys become two tiers:

| tier      | token            | value     | was                  | what it draws                                                                                                    | held to                           |
| --------- | ---------------- | --------- | -------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| primary   | `text.primary`   | `#e6e4ec` | as drawn             | titles, names, the tab in front, buttons                                                                         | 4.5:1 on every ground             |
| secondary | `text.secondary` | `#c9c7d2` | as drawn             | body text                                                                                                        | 4.5:1 on every ground             |
| muted     | `text.muted`     | `#a09eab` | as drawn             | chrome labels, the rails' idle icons                                                                             | 4.5:1 on every ground             |
| **quiet** | `text.quiet`     | `#91909d` | `#8a8896`            | informational text: line numbers, placeholders, counts, paths, group labels ("Earlier"), the status bar's folder | 4.5:1 on every ground             |
| **faint** | `text.faint`     | `#74727f` | `#6f6d7a`, `#55535f` | what is not words: a tab's ×, separators ("/"), the pin, chevrons, checkbox rings; disabled words                | 3:1 on every ground (WCAG 1.4.11) |

"Every ground" is the window `#17171b`, a panel `#1d1d22`, code `#202026`, a card `#24242a`,
the row under the pointer `#2a2a31` and the selected row, which Purlis draws at `#2a2936`, one
step under the frame's `#2b2a37`, so quiet and faint clear it. The theme check measures every
tier on each of them, and on the strips, the status bar and a control, in every appearance.
Informational text that the frame draws in `#6f6d7a` (placeholders, heading counts, paths,
"Earlier", the status bar's folder) and the line numbers in `#55535f` take quiet, not faint.

### Accent: lavender

| colour                   | uses | where it is used                                                                               | proposed token (proposal)                                           |
| ------------------------ | ---- | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `#a99cf0`                | 2    | the workspace's dot `6:25`, the status bar's workspace dot `6:611`                             | `accent.base`, `focus.ring`, `list.selected-edge`                   |
| `#c3baf5`                | 25   | text on an accent wash ("Update 0.4.2-dev.781", "Ask a chat"); a rail's chosen icons; JSON keys | **no token** for accent text; JSON keys: syntax, **no token**       |
| `#b3a8f2`                | 2    | text buttons on notice cards ("Review hosts", "Review access")                                 | **no token** for links; nearest `accent.base`                       |
| `rgba(169,156,240,0.10)` | 1    | the chosen repo row `6:538`                                                                    | `list.selected` (as `#a99cf01a`)                                    |
| `rgba(169,156,240,0.12)` | 2    | the update pill `6:21`, the "Ask a chat" button `6:126`                                        | `accent.surface` (as `#a99cf01f`)                                   |
| `rgba(169,156,240,0.14)` | 4    | a rail's chosen buttons `6:7813`, `6:7826`, `6:7882`, `6:7889`                                 | **no token** for a rail's chosen button (as `#a99cf024`); nearest `accent.surface` |

### Amber, green and code

| colour                   | uses | where it is used                                                                                         | proposed token (proposal)                                     |
| ------------------------ | ---- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `#e5c08a`                | 8    | the unsaved dot in the title bar, "1 unpushed · ↓20", "Reinit", "Update layout", "6 warnings"; JSON `true` | `state.waiting`; JSON literals: syntax, **no token**          |
| `rgba(229,192,138,0.07)` | 1    | the Reinit notice card `6:524`                                                                           | **no token** for a waiting wash (as `#e5c08a12`); the pattern of `danger.wash` |
| `rgba(229,192,138,0.10)` | 1    | the unpushed pill `6:16`                                                                                 | **no token** (as `#e5c08a1a`)                                 |
| `rgba(229,192,138,0.14)` | 1    | the "Update layout" button `6:528`                                                                       | **no token** (as `#e5c08a24`)                                 |
| `#8fc79a`                | 2    | "Nothing is waiting on you" and its dot `6:502`                                                          | `state.success`                                               |
| `#9fcfa8`                | 15   | JSON string values                                                                                       | syntax, **no token**; nearest `terminal.ansi.green`           |
| `#77757f`                | 43   | JSON punctuation (`:`, `,`, braces)                                                                      | syntax, **no token**; nearest `terminal.ansi.bright-black`    |

The 8-digit hex in brackets is the nearest byte to the frame's alpha (0.10 → `1a`, 0.12 →
`1f`, 0.14 → `24`, 0.07 → `12`); the frame's own values are the decimals.

The frame draws no red, no running blue, no diff colours, no file-type icon hues and no
terminal. Those tokens have no foundation value and are left to the Purlis theme.

## Type

Two families: **IBM Plex Sans** for the interface, **JetBrains Mono** for code and paths.
"normal" line height is the font's own (a 13 px line of Plex Sans is 17 px high in the frame).

| family         | weight         | size    | line height | colour    | where it is used                                                      |
| -------------- | -------------- | ------- | ----------- | --------- | --------------------------------------------------------------------- |
| IBM Plex Sans  | SemiBold 600   | 13      | normal      | `#e6e4ec` | panel titles: Chats, Notices, Repos, Todos; the editor's file name    |
| IBM Plex Sans  | Medium 500     | 13      | normal      | `#a09eab` | the title bar's project name ("purlis-plane")                         |
| IBM Plex Sans  | Medium 500     | 13      | normal      | `#e6e4ec` | the workspace tab's name; the chosen repo row's name                  |
| IBM Plex Sans  | Medium 500     | 13      | 20.15       | `#e5c08a` | "Reinit", the lead word of a notice                                   |
| IBM Plex Sans  | Regular 400    | 13      | normal      | `#c9c7d2` | todo rows, repo rows                                                  |
| IBM Plex Sans  | Regular 400    | 13      | 20.15       | `#c9c7d2` | a notice's sentence; its detail runs on in `#8a8896`                  |
| IBM Plex Sans  | Regular 400    | 13      | 20.8        | `#8a8896` | the Chats panel's empty state, centred                                |
| IBM Plex Sans  | Regular 400    | 13      | normal      | `#6f6d7a` | field placeholders; heading counts; "No branches cut here"            |
| IBM Plex Sans  | Regular 400    | 13      | normal      | `#8a8896` | the panels' collapse chevrons (⌄)                                     |
| IBM Plex Sans  | Regular 400    | 12.5    | normal      | `#8a8896` | a file tab behind; `#e6e4ec` for the tab in front; `#6f6d7a` for ×   |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#a09eab` | title bar pills, toolbar buttons, filter pills                        |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#e6e4ec` | filled buttons ("Got it", "Add to context")                           |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#c3baf5` | buttons and pills on an accent wash                                   |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#b3a8f2` | text buttons on notice cards                                          |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#e5c08a` | the unpushed pill, "Update layout"                                    |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#8fc79a` | "Nothing is waiting on you"                                           |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#8a8896` | status bar items; the repo row's note; `#c9c7d2` for its workspace    |
| IBM Plex Sans  | Regular 400    | 12      | normal      | `#6f6d7a` | the workspace tab's ×                                                 |
| IBM Plex Sans  | Regular 400    | 11      | normal      | `#8a8896` | count badges; scope segments; `#e6e4ec` for the chosen one            |
| IBM Plex Sans  | Regular 400    | 11      | normal      | `#6f6d7a` | the Todos group label "Earlier"                                       |
| IBM Plex Sans  | Regular 400    | 16      | normal      | `#a09eab` | the `+` of the workspace strip and the file tab strip                 |
| JetBrains Mono | Regular 400    | 12.5    | 21          | see Code  | the editor's code and line numbers                                    |
| JetBrains Mono | Regular 400    | 12      | normal      | `#6f6d7a` | the file's path in the toolbar; the plane's folder in the status bar  |

Proposals against today's geometry (`app/src/theme/geometry.css`, at its 14 px root): 11, 12, 13
and 16 are `--font-size-xs`, `-sm`, `-md` and `-lg` exactly; **12.5 has no size token**. 600 is
`--font-weight-strong` and 500 is `--font-weight-medium` (#2317). The line heights 20.15
(13 × 1.55), 20.8 (13 × 1.6) and 21 (12.5 × 1.68) are the frame's; which line-height tokens they
become is the theme's work.

Code colours (JetBrains Mono 12.5 / 21): keys `#c3baf5`, strings `#9fcfa8`, literals
(`true`) `#e5c08a`, punctuation `#77757f`, line numbers `#55535f`. The frame's export shows the mono
face's ligatures on (`>=` draws as ≥, `||` joined).

## Spacing

The distinct padding and gap values, in px. Today's `--space-*` scale (at 14 px) is 0, 2, 4,
6, 8, 12, 16, 24, 32.

| value | where                                                                                  | on today's scale (proposal) |
| ----- | -------------------------------------------------------------------------------------- | --------------------------- |
| 1     | count badges' vertical padding                                                         | **off scale**               |
| 2     | the scope switch's inset; tabs' gap; rails' bottom padding                             | `--space-1`                 |
| 3     | scope segments' vertical padding                                                       | **off scale**               |
| 4     | pills' vertical padding; workspace strip gap; toolbar button gap; rails' top padding   | `--space-2`                 |
| 5     | buttons' vertical padding                                                              | **off scale**               |
| 6     | gaps between pills and buttons; rail gap; workspace strip bottom; tab strip top        | `--space-3`                 |
| 7     | file tabs' vertical padding; workspace count's horizontal padding                      | **off scale**               |
| 8     | body padding and region gaps; segments' and counts' horizontal padding; field vertical padding; panel gaps | `--space-4` |
| 10    | pills', buttons' and fields' horizontal padding; toolbar padding; card gap; todo gap   | **off scale**               |
| 12    | card padding; file tab horizontal padding; workspace tab horizontal padding; Chats gap | `--space-5`                 |
| 14    | panel padding; title bar horizontal padding; status bar gap                            | **off scale**               |
| 16    | title bar gap; status bar horizontal padding                                           | `--space-6`                 |
| 18    | the code gutter's right padding                                                        | **off scale**               |
| 24    | empty state vertical padding; a child repo's indent                                    | `--space-7`                 |
| 36    | the "No branches" line's indent                                                        | **off scale**               |
| 56    | the workspace strip's left padding (lines up with the main column)                     | **off scale**               |

## Sizes

| size          | what                                                                           |
| ------------- | ------------------------------------------------------------------------------ |
| 44            | title bar height                                                               |
| 40            | workspace strip height; rail width                                             |
| 36            | rail button                                                                    |
| 30            | status bar height                                                              |
| 30.5          | file tab height                                                                |
| 29            | workspace tab height                                                           |
| 28            | the `+` buttons                                                                |
| 26            | buttons (toolbar and cards)                                                    |
| 24            | pills; icons; the Chats header row                                             |
| 21            | a code line                                                                    |
| 20            | a scope segment                                                                |
| 16            | count badges                                                                   |
| 14            | todo checkbox                                                                  |
| 6             | status dots                                                                    |
| 250, 772, 290 | Chats, editor, Notices widths at 1440 (the editor takes the rest)              |
| 520, 250      | upper and lower row heights at 904                                             |

## Radii

| radius | where                                                              | on today's scale (proposal)   |
| ------ | ------------------------------------------------------------------ | ----------------------------- |
| 3      | the 6 px status dots (a circle)                                    | `--radius-sm`                 |
| 5      | the scope switch's chosen segment                                  | **no token**                  |
| 7      | buttons; the scope switch's track; the 14 px checkbox (a circle)   | **no token**                  |
| 8      | fields; the workspace tab; a file tab's top corners; repo row; `+` | `--radius-xl`                 |
| 9      | rail buttons                                                       | **no token**                  |
| 10     | notice cards                                                       | **no token**                  |
| 12     | the five panels; the window itself                                 | **no token**                  |
| 999    | pills and count badges                                             | `--radius-full` (9999)        |

## Borders, shadows and effects

- **One border**: the todo checkbox, 1.5 px solid `#55535f`, round. No hairlines divide regions
  or tabs; regions are told apart by their surface step alone.
- **No shadows**, no blur, no opacity on layers. Everything translucent is a fill's alpha
  (listed under Colours). Panels and the editor clip their content to their corners.

## Icons

Eleven icon instances, each named as in Untitled UI Icons (the file holds them as local
components). Every one is 24 × 24 on a 24 viewBox, stroked, not filled: stroke 2, round caps,
`fill="none"`. Chosen buttons draw the icon in `#c3baf5` on `rgba(169,156,240,0.14)`; the
others draw it in `#a09eab` on nothing.

| rail  | position | name                 | state in the frame | Figma component |
| ----- | -------- | -------------------- | ------------------ | --------------- |
| left  | top      | `annotation`         | chosen             | `6:5245`        |
| left  | top      | `dataflow-01`        | idle               | `6:2722`        |
| left  | top      | `search-refraction`  | idle               | `6:4089`        |
| left  | bottom   | `folder`             | chosen             | `6:6155`        |
| left  | bottom   | `dataflow-03`        | idle               | `6:2730`        |
| right | top      | `bell-01`            | chosen             | `6:4351`        |
| right | top      | `users-01`           | idle               | `6:4651`        |
| right | top      | `key-01`             | idle               | `6:2406`        |
| right | bottom   | `box`                | chosen             | `6:6235`        |
| right | bottom   | `file-check-02`      | idle               | `6:6063`        |
| right | bottom   | `clock-fast-forward` | outside the rail   | `6:5965`        |

`clock-fast-forward` (`6:7891`) sits at y 782 in a rail 778 high, so the frame does not draw
it: its export shows the status bar where it would be.

The other glyphs are text, not icons: `×`, `+`, `⌄`, `›`, `•`, `↓`, `·`.

## Fonts and icons

The operator decided on 2026-10-17: Purlis uses IBM Plex Sans and JetBrains Mono, approved for
bundling under the SIL Open Font License (#2336), and draws its icons from Lucide (ISC) at the
frame's 24 px and 2 px round stroke in place of Untitled UI Icons (#2337). Both are bundled in
the Purlis theme folder, `crates/purlis-core/src/theme/built-in/purlis/`, with their licences
in its `LICENSE`.

| asset             | what the frame uses                     | used for       | status                          |
| ----------------- | --------------------------------------- | -------------- | ------------------------------- |
| IBM Plex Sans     | Regular 400, Medium 500, SemiBold 600   | the interface  | bundled, OFL (#2336), with each weight's italic (#2709) |
| JetBrains Mono    | Regular 400                             | code and paths | bundled, OFL (#2336), with its Italic and Bold (#2709) |
| Untitled UI Icons | eleven icons, by name                   | the rails      | not bundled: Lucide (ISC) draws the rails instead (#2337) |

## The frame and the tables agree

Points read off the Figma frame's 2× export on 2026-10-16 (each at twice the 1× position),
against the colour the table gives the node there.

| point                                  | table       | export      |
| -------------------------------------- | ----------- | ----------- |
| under the window, `6:2`                | `#121215`   | `#121215`   |
| the window, `6:6`                      | `#17171b`   | `#17171b`   |
| Chats panel, `6:58`                    | `#1d1d22`   | `#1d1d22`   |
| editor code, `6:130`                   | `#202026`   | `#202026`   |
| notice card, `6:507`                   | `#24242a`   | `#24242a`   |
| "Add to context", `6:128`              | `#2a2a31`   | `#2a2a31`   |
| "Got it", `6:512`                      | `#2f2f37`   | `#2f2f37`   |
| status bar, `6:609`                    | `#141417`   | `#141417`   |
| status bar dot, `6:611`                | `#a99cf0`   | `#a99cf0`   |
| chosen repo row, 0.10 lavender on `#1d1d22` | `#2b2a37` | `#2b2a37` |
| Reinit card, 0.07 amber on `#1d1d22`   | `#2b2829`   | `#2b282a` (rounding) |
| chosen rail button, 0.14 lavender on `#17171b` | `#2b2a39` | `#2c2a39` (rounding) |
| `clock-fast-forward`'s slot            | not drawn   | `#141417`, the status bar |
