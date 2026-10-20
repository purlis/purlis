# Purlis preview theme vs the Figma design: the gap

Read 2026-10-20. Figma is the source of truth (operator ruling 2026-10-20: the default theme
must be the Figma design; this replaces "Figma is a one-time foundation, not a pixel sync").

**Sources**

- Figma file `A5tZlp0AjLs1VeFN5TVgXW`, read only: `get_metadata` (pages, frame tree),
  `get_variable_defs` on `6:2`, `get_screenshot` of `6:2`, `get_design_context` of `6:2`
  (full React/Tailwind dump, 105 KB, every value below is from it).
- purlis/purlis `main` at `47dab253b050afaf53eef11430cff4e54df29c2a`. The sandbox refuses
  `git clone` (it denies writes to `.git/config` and `.git/hooks`), so I fetched the files
  through `gh api .../contents?ref=<sha>` into the scratch folder instead. File:line references
  are to that sha.
- The repo's own record of the same frame: `docs/design/foundation/README.md` ("read once from
  Figma", 2026-10-16). I checked it against today's read: it still matches node for node, with
  one miss (the editor's border, see section 1.6).

## 0. The headline

1. **The Figma file has no variables, no styles and no component sets.**
   `get_variable_defs(6:2)` returns `{}`. The single page `0:1` holds one frame, `6:2` "Purlis"
   (1440 × 904), which is one dark app screen made of layers called `Container`/`Text`. The only
   local components are the 11 Untitled UI icons. So the Figma file defines no tokens to map.
   Each "variable" in section 1 is a raw value the frame uses, and each "component" in section 2
   is one instance on that screen. **None has variants, sizes or states**: no hover, pressed,
   focus, disabled, error or light appearance is drawn anywhere.
2. **Colour is close.** The Purlis palette is the frame's palette byte for byte
   (`crates/purlis-core/src/theme/built-in/purlis/theme.json:4-53`). The gaps are roles, not
   values. The frame uses six text greys, but purlis has three text tokens plus `placeholder`.
   It uses three lavender washes and three amber washes, but purlis has one accent surface and
   no amber wash. And a single token paints two Figma colours in several places (`surface.deep`
   paints both the status bar and the rails, `border.subtle` is both a hairline and the "Got it"
   fill).
3. **Shape and structure are far.** Figma's window is rounded panels floating on the window
   with 8 px gutters and **no hairlines**. Purlis draws flush strips and panes split by 1 px
   dividers. Its tabs are equal-share square cells (`border-radius: 0` hard-coded,
   `app/src/App.css:441`). Every control has a 1 px `border.strong` bezel
   (`app/src/primitives/controls.css:45,174,361,410`). Purlis also caps every corner token
   (`radius.md` ≤ 10, `radius.lg` ≤ 14, `radius.xl` ≤ 18, `crates/purlis-core/src/theme/geometry.rs:193-204`).
   And it has no component tokens: ADR 0097 §1 says they are "in the vocabulary", but
   `vocabulary.rs` has none and the primitive classes have no `--btn-*` aliases. So a theme
   cannot remove a bezel, round a tab or a panel, or give one control a corner other than its
   class's.
4. **The built-in Purlis theme is held to "no warnings"** (`docs/design-system.md:370`, test
   `purlis_tests.rs`). Figma's two quietest greys (`#6f6d7a`, `#55535f`) and its `#8a8896` on
   a card fall under the 4.5:1 text floor. So matching Figma means either relaxing that test
   or adding roles whose floor is lower (section 1.2).

---

## 1. Every Figma value → purlis token

Status key: **matched** (same value reaches the same role), **different** (purlis paints that
role with another value, both given), **missing** (no purlis token can express it).
Purlis values are the Purlis theme's `dark` appearance (`theme.json:55-232`) unless said.

### 1.1 Surfaces

| Figma value | where (node) | purlis token | status |
| --- | --- | --- | --- |
| `#121215` | behind the window `6:2` | `window.background` = `behind` (`theme.json:207`) | matched |
| `#17171b` | window `6:6`; title bar, workspace strip, rails sit on it; fields `6:70`, `6:556`; scope track `6:63` | `surface.base`, `surface.sunken`, `layer.project`, `layer.workspace` (`theme.json:59-60,81-82`) | matched for the window and the fields. **Different for the title bar and the rails**: purlis paints both with `surface.deep` = `#141417` (`App.css:5805` `.title-bar`, `App.css:2723` `.activity-column`). |
| `#141417` | status bar `6:609` | `surface.deep` (`theme.json:61`) | matched for the status bar (`App.css:5295`). One token paints the status bar, the title bar and the rails, but Figma wants the status bar alone at `#141417`. **Missing**: a separate status-bar or rail surface. |
| `#1d1d22` | the five panels `6:58`, `6:81`, `6:501`, `6:532`, `6:550` | `surface.raised`, `layer.chat` (`theme.json:62,83`) | matched |
| `#202026` | editor toolbar `6:112`, code `6:130`, front file tab `6:104` | `tab.active`, `terminal.background` (`theme.json:77,115`) | matched for the code and the front tab. **Missing**: an editor toolbar surface. Purlis also paints the code gutter with `surface.raised` `#1d1d22` (`app/src/editor/look.ts:41-44`), where Figma wants `#202026`. |
| `#24242a` | workspace tab `6:24`; filter pills `6:74`/`6:76`; notice cards `6:507`/`6:516`; repo count `6:546` | `surface.overlay`, `control.base`, `layer.selected` (`theme.json:63,65,84`) | matched for the tab, pills and cards. **Different** for the repo count: purlis `control.count` = `#2a2a31` (`theme.json:68`). Figma uses two count fills, `#24242a` for the repo count and `#2a2a31` for the workspace count, and purlis has one token. |
| `#2a2a31` | workspace count `6:29`; chosen scope segment `6:64`; "Add to context" button `6:128` | `surface.hover`, `control.hover`, `control.count` (`theme.json:64,66,68`) | matched for the count. **Different** for the button: Figma draws a filled button at rest in `#2a2a31`, but purlis's resting button is `control.base` `#24242a` (`controls.css:48`). **Different** for the segment: purlis's chosen segment is `accent.surface` (`App.css:7854`, `controls.css:440`). |
| `#2f2f37` | "Got it" buttons `6:512`, `6:520` | `border.subtle`, `layer.divider` = `card-button` (`theme.json:72,85`) | **missing as a fill.** The colour is in the palette, but purlis spends it on hairlines. No token exists for a button on a card. |
| `#2c2c2c` (1 px border) | the editor panel `6:81` | none | **missing.** The colour is not in the palette, and the foundation record says "one border" (`docs/design/foundation/README.md:213`), so it missed this one. |

### 1.2 Text

| Figma value | where | purlis token | status |
| --- | --- | --- | --- |
| `#e6e4ec` | titles, front tab, workspace name, filled buttons | `text.primary` (`theme.json:69`) | matched |
| `#c9c7d2` | body: todos, notice sentence, repo rows, status-bar workspace | `text.secondary` (`theme.json:70`) | matched |
| `#a09eab` | chrome labels: title bar name, pills, toolbar ghost buttons, `+`, idle rail icons | `text.muted` = `label` (`theme.json:71`) | matched where purlis draws muted chrome. |
| `#8a8896` | tabs behind, scope segments, counts, empty states, status-bar items, chevrons, notice detail | `placeholder` = `quiet` (`theme.json:196`) | **different.** Purlis draws unselected tabs, segments and counts as `text.muted` `#a09eab` (`App.css:443`, `App.css:2929`) or `text.secondary` (`.ui-level` `App.css:7822`, `.count-pill` `App.css:596`). The repo chose `#a09eab` for `text.muted` on purpose, because `#8a8896` measures 4.44 on a card (`docs/design-system.md:385-386`). **Missing**: a "quiet" text tier. |
| `#6f6d7a` | placeholders, tab `×`, heading counts, paths, "Earlier", status-bar folder | none (not in the palette) | **missing.** Its contrast is 3.53 on the window, 3.31 on a panel and 3.05 on a card, so as `placeholder` it fails `p("placeholder","surface.sunken",TEXT)` (`check.rs:597`). Purlis's placeholder is `#8a8896`. |
| `#55535f` | editor line numbers; todo checkbox ring | `border.strong`, `tree.guide` (`theme.json:73,80`) | matched as an edge. **Different** for line numbers: purlis gutter text is `text.muted` `#a09eab` (`look.ts:42`). **Missing**: a gutter token. Its contrast is 2.15 on code. |
| `#c3baf5` | text on an accent wash ("Update…", "Ask a chat"), chosen rail icons, JSON keys | `syntax.property` (`theme.json:150`) | matched for JSON keys. **Missing**: an accent-text token. Purlis's primary button text is `text.primary` (`controls.css:62`), and a chosen rail icon is `text.primary` (`App.css:2790`). |
| `#b3a8f2` | text buttons on notice cards ("Review hosts") | `markdown.link` (`theme.json:188`) | **missing outside markdown.** No link/text-button token exists for the UI. |
| `#e5c08a` | amber words: unsaved dot, unpushed, Reinit, Update layout, 6 warnings; JSON `true` | `state.waiting`, `syntax.number`, `syntax.constant` (`theme.json:108,145-146`) | matched |
| `#8fc79a` | "Nothing is waiting on you" + dot | `state.success` (`theme.json:111`) | matched |

Contrast of Figma's greys, computed here with WCAG relative luminance:

| fg | window `#17171b` | panel `#1d1d22` | code `#202026` | card `#24242a` |
| --- | --- | --- | --- | --- |
| `#a09eab` | 6.79 | 6.37 | 6.15 | 5.86 |
| `#8a8896` | 5.14 | 4.83 | 4.66 | **4.44** |
| `#6f6d7a` | **3.53** | **3.31** | **3.20** | **3.05** |
| `#55535f` | **2.37** | **2.23** | **2.15** | **2.05** |

Under the engine these are warnings, and the theme is still drawn (`crates/purlis-core/docs/themes.md:157-163`).
They block only because the built-in must pass with zero warnings.

### 1.3 Accent and washes

| Figma value | where | purlis token | status |
| --- | --- | --- | --- |
| `#a99cf0` | workspace dot `6:25`, status dot `6:611` | `accent.base`, `focus.ring`, `list.selected-edge` (`theme.json:74,76,79`) | matched |
| lavender 0.12 | update pill `6:21` (on window), "Ask a chat" `6:126` (on code) | `accent.surface` = `#292735`, an opaque flattening over `#17171b` (`theme.json:75`) | matched on the window. **Different** on the code surface, because the flattened colour is not a wash (Figma would give about `#2b2a3b` there). |
| lavender 0.10 | chosen repo row `6:538` | `list.selected` = `#2b2a37` (`theme.json:78`) | the colour matches. **Different in shape**: purlis adds a 2 px `list.selected-edge` inset (`primitives/collections.css:254`), and Figma has no edge and a radius of 8. |
| lavender 0.14 | chosen rail buttons `6:7813`, `6:7826`, `6:7882`, `6:7889` | none | **missing.** Purlis marks a chosen rail tab with a 2 px accent edge (`App.css:2793-2799`), not a wash. |
| amber 0.07 | Reinit card `6:524` | none | **missing** (no waiting wash; `state.waiting-glow` `#e5c08a59` is a glow, `theme.json:109`) |
| amber 0.10 | unpushed pill `6:16` | none | **missing** |
| amber 0.14 | "Update layout" button `6:528` | none | **missing** |
| green `#9fcfa8` | JSON strings | `syntax.string` (`theme.json:144`) | matched |
| `#77757f` | JSON punctuation | `syntax.punctuation` (`theme.json:157`) | matched |

### 1.4 Type

| Figma value | where | purlis token | status |
| --- | --- | --- | --- |
| IBM Plex Sans 400/500/600 | the interface | `fonts.ui` (`theme.json:593-624`); `font-weight.*` 400/500/600 (`geometry.rs:152-175`) | matched |
| JetBrains Mono 400 | code, paths | `fonts.mono`, `fonts.terminal` (`theme.json:625-660`) | matched |
| 11 / 12 / 13 / 16 px | counts, segments / pills, buttons, status bar / titles, rows / `+` | `font-size.xs` / `sm` / `md` / `lg` (`geometry.rs:103-142`) | the scale matches. **Different in use**: purlis buttons, fields, selects and segments use `font-size.base` = 14 (`controls.css:49,178,424`), where Figma uses 12 for buttons and 13 for fields. |
| 12.5 px | file tabs, editor code | none | **missing** (no step between `sm` 12 and `md` 13) |
| line height "normal" (Plex ≈ 1.3) | almost every label | `line-height.tight` 1.2 / `body` 1.55 | **different.** The theme sets `line-height.body` 1.55 (`theme.json:217`) on `:root`, so every label gets Figma's notice leading. |
| 1.55 (20.15 / 13) | notice sentences | `line-height.body` 1.55 | matched |
| 1.6 (20.8 / 13) | Chats empty state | none | **different** (1.55) |
| 21 / 12.5 = 1.68 | code lines | none for the editor (`terminal.line-height` 1 is the terminal's, `theme.json:210`) | **missing** |

### 1.5 Spacing and sizes

| Figma value | where | purlis | status |
| --- | --- | --- | --- |
| 2, 4, 6, 8, 12, 16, 24 | gaps, paddings | `space.1`–`space.7` (`geometry.rs:85-93`); the theme sets none | matched as a scale |
| 1, 3, 5, 7, 10, 14, 18, 36, 56 | badge v-pad, segment v-pad, button v-pad, tab v-pad, button/field/pill h-pad, panel pad, gutter, indents | none | **missing.** Off the scale. They are reachable only by moving a whole step, and a step is shared by many components (e.g. `space.4` is both the button's h-pad and the body gaps). |
| 26 | buttons | `size.control` 26 (`theme.json:215`) | matched |
| 24 | pills, Chats header row | `size.chrome` 24 (`theme.json:216`) | matched |
| 44 / 40 / 30 | title bar / workspace strip / status bar heights | none. The title bar is `min-height: 2.1rem` (`App.css:5802`), the status line is padding-driven (`App.css:5291`) | **missing** |
| 40 / 36 | rail width / rail button | `.activity-column` `width: 2.5rem` (40, hard-coded, `App.css:2721`); `.activity-tab` full width × `2.5rem` | width matched by literal. Button **different** (40 × 40 against 36 × 36) and **missing** as a token |
| 24 icon | rail icons | `.activity-tab svg` 1.25rem = 20 (`App.css:2772`) | **different**, **missing** as a token |
| 30.5 / 29 | file tab / workspace tab heights | cells stretch to their strip | **different** |
| 6 dot | status dots | `.live-dot` `space-3` = 6 (`App.css:613`); `.workspace-mark` 0.5rem = 8 (`App.css:628`) | live dot matched; workspace mark **different** |
| 14 checkbox, 1.5 px ring | todo checkbox `6:561` | `.ui-checkbox` = `size.control × 3/7` (≈ 11 at 26), 1 px `border.subtle`, `radius.sm` (`controls.css:297-312`) | **different**. No border-width token, so the 1.5 px ring is **missing**. |

### 1.6 Radius, borders, elevation

| Figma value | where | purlis token | status |
| --- | --- | --- | --- |
| 3 | 6 px dots | `radius.sm` 3 | matched |
| 5 | chosen scope segment `6:64` | none | **missing** |
| 7 | buttons, scope track `6:63`, checkbox (a circle) | `radius.md` 7 (`theme.json:212`) | matched for buttons. **Different** for the track, because `.ui-segmented`/`.ui-levels` use `radius.lg` = 10 (`controls.css:411`, `App.css:7816`) |
| 8 | fields `6:70`; workspace tab `6:24`; file tab top corners `6:83`…; repo row `6:538`; `+` `6:34` | none (the theme moved `radius.xl` from 8 to 12) | **missing.** Fields draw `radius.md` 7 (`controls.css:175`). Tabs draw 0, hard-coded (`App.css:441`, `.closer` `:517`). A selected row draws 0 (`collections.css:210`). |
| 9 | rail buttons | none | **missing** (`.activity-tab` has no radius) |
| 10 | notice cards | `radius.lg` 10 (`theme.json:213`) | the token matches. **Different in use**: `.notice-list` draws `radius.md` (`App.css:2899`), and `radius.lg` draws dialogs, menus and popovers (`primitives/overlays.css:34`), which Figma does not draw. |
| 12 | the five panels; the window | `radius.xl` 12 (`theme.json:214`), but it draws only the Welcome doors (`App.css:5065`) | **missing for panels.** The repo says "Panels' 12 has no token yet" (`docs/design-system.md:390`). Window corners are the OS's. |
| 999 | pills, counts | `radius.full` | matched |
| no hairlines anywhere (except `6:81`'s `#2c2c2c`) | between regions, tabs, controls | purlis draws them: strip cells `layer.divider` (`App.css:381`); workspace strip `::after` `border.subtle` (`App.css:285-292`); rail `border.subtle` (`App.css:2726`); title bar and status line `border.subtle` (`App.css:5806`, `:5296`); every control `border.strong` (`controls.css:45,174,361,410`); `.ui-levels` and segments (`App.css:7815,7830`); gutter (`look.ts:44`); notices (`App.css:2898`); overlays (`overlays.css:33`) | **different.** No token turns a border off or sets its width. A theme can only paint it the ground's colour, and that trips the `border.*` floors (`check.rs:304-305`). |
| no shadow | everywhere | `elevation.raised-*` 0 (`theme.json:218-219`) | matched. `elevation.overlay` has no Figma value, because no overlay is drawn. |
| no focus ring drawn | — | `focus-ring.*` | no Figma value |

---

## 2. Components on the frame

There are no component frames: every row below is the one instance on `6:2`. "Expressible"
asks whether the Purlis theme (`theme.json`, within `geometry.rs` bounds and `check.rs` floors)
can make purlis draw it.

### 2.1 Title bar (`6:7`)

- **Anatomy**: the plane name (Plex Medium 13, `#a09eab`) with an amber `•` (`6:13`). On the
  right, three pills (`6:15`, gap 6), each 24 high with padding 4/10, `radius 999`, 12 px text:
  - status pill: amber 0.10 wash, amber text (`6:16`);
  - ghost pill "About": no fill, `#a09eab` text (`6:19`);
  - accent pill "Update …": lavender 0.12 wash, `#c3baf5` text (`6:21`).

  The bar sits on the window (`#17171b`), with no bottom border.
- **Purlis**: `.title-bar` holds the **projects strip** (equal-share tabs), not a plane name
  (`App.css:5796-5820`). It is painted `surface.deep` with a `border.subtle` bottom. `.title-about`
  and `.status-update` are `control.base` boxes with a 1 px `border.subtle` and `radius.md`
  (`App.css:5850-5862`, `:5402-5413`).
- **Expressible**: no.
- **Missing**: a title-bar surface separate from `surface.deep`; pill-shaped chrome buttons (the
  radius is hard-wired to `radius.md`); a border-off switch; amber-wash and accent-text tokens;
  title bar height (44).

### 2.2 Workspace strip and workspace tab (`6:23`, `6:24`)

- **Anatomy**: the strip has padding `0 8 6 56`, gap 4, and no divider. The tab is a **chip**:
  `#24242a`, `radius 8`, padding 6/12, gap 8. Inside it are a 6 px accent dot (`6:25`), the name
  (Medium 13, `#e6e4ec`), a count badge (`#2a2a31`, radius 999, padding 1/7, 11 px `#8a8896`) and
  a `×` (12 px `#6f6d7a`). The `+` is 28 × 28, radius 8, 16 px `#a09eab` (`6:34`).
- **Purlis**: `.workspaces-strip .workspace` is an equal-share cell (`flex: 1 1 0`, a
  `min-width` floor) with a 1 px `layer.divider` (`App.css:371-382`). The tab button has
  `border-radius: 0` (`App.css:441`), and the selected one fills `layer.selected` edge to edge
  (`App.css:413-420`). There is no gap between tabs. The strip has a `border.subtle` rule under
  it (`App.css:285`). The count pill is `control.count` with `text.secondary` (`App.css:590-605`),
  and the workspace mark is 8 px (`App.css:628`).
- **Expressible**: no. The colours are, but the shape is not.
- **Missing**: tab corner radius (hard-coded 0); a tab sizing mode (content-sized against equal
  share, which is a layout rule, not a token); a tab gap; a divider-off switch; a quiet text
  tier for the count and the `×`.

### 2.3 Activity rails (`6:7814`, `6:7881`)

- **Anatomy**: a 40 wide column on the window with no border, padding 4/2, gap 6. Its buttons
  are 36 × 36 with radius 9. A chosen button has a lavender 0.14 fill and a `#c3baf5` icon; an
  idle one has no fill and a `#a09eab` icon. Icons are 24 px with a 2 px stroke. A spacer splits
  the top group from the bottom group, with no divider line.
- **Purlis**: `.activity-column` is `surface.deep` with a `border.subtle` edge (`App.css:2717-2731`)
  and an `.activity-divider` hairline (`:2736`). `.activity-tab` is full width × 40 with no
  radius, and its icon is 20 px (`:2757-2775`). Chosen means `text.primary` plus a 2 px inset
  accent edge (`:2789-2799`).
- **Expressible**: no.
- **Missing**: a rail surface; a chosen-rail-button fill (accent wash); accent icon colour;
  button radius (9); button size and icon size tokens; a divider-off switch.

### 2.4 Panels (`6:58`, `6:81`, `6:501`, `6:532`, `6:550`)

- **Anatomy**: `#1d1d22`, radius 12, padding 14, internal gap 8–12, 8 px gutters between panels
  and around the body (`6:37` padding `0 8 8`, gap 8). The header is a title (SemiBold 13
  `#e6e4ec`), an optional count (`#6f6d7a`) and a chevron `⌄` (`#8a8896`). The editor panel adds
  a 1 px `#2c2c2c` border (`6:81`).
- **Purlis**: the regions and panes are flush. `.panel-halves` splits them with a 1 px separator
  and `space-2` padding (`App.css:2629-2645`). `.pane` has a transparent 1 px border that turns
  `accent.base` when focused (`App.css:1145-1158`), and `.half-head` has `space-2`/`space-3`
  padding (`:2678`). No region has a corner.
- **Expressible**: no (colour only).
- **Missing**: a panel radius role (`radius.xl` exists but is not used on panels); panel gutter
  and inset (a layout gap between regions); panel padding 14 (off scale); a panel or editor
  border colour.

### 2.5 Editor file tabs (`6:82` strip, `6:83`…`6:104` tabs)

- **Anatomy**: the strip has padding 6/6/0 and gap 2. A tab is content-sized, padding 7/12, gap
  8, **top corners 8**. The tab behind has no fill and 12.5 px `#8a8896` text; the front tab is
  `#202026` (it joins the toolbar below it) with `#e6e4ec` text. The `×` is `#6f6d7a`. The `+`
  is 28 × 28 with radius 8.
- **Purlis**: `.tabs .tab` is an equal-share cell with a max width of 15rem and a divider
  (`App.css:371-404`). `.tabs [role=tab]` is square with `font-size.md` 13 and `text.muted`
  (`App.css:427-455`). The front tab is `layer.selected` (`App.css:413-420`), not `tab.active`.
  The `×` turns `danger.surface` on hover (`App.css:509-528`).
- **Expressible**: no.
- **Missing**: per-corner tab radius (top only); the 12.5 size; content-sized tabs; a tab gap; a
  front-tab fill that meets the toolbar (purlis paints the front tab `layer.selected` `#24242a`,
  but Figma wants `#202026` = `tab.active`).

### 2.6 Editor toolbar and code (`6:112`, `6:130`)

- **Anatomy**: toolbar `#202026`, padding 10/14, gap 10. It holds the file name (SemiBold 13),
  the path (Mono 12 `#6f6d7a`) and five buttons (see 2.7). Code is Mono 12.5 on a 21 px line.
  The gutter is 52 wide, `#55535f` numbers, no rule, the same fill as the code. There is no
  active-line band.
- **Purlis**: the code is `terminal.background`. The gutter is `text.muted` on `surface.raised`
  with a `border.subtle` rule, and the active line is `surface.hover` (`look.ts:30-46`).
- **Expressible**: partly. Syntax colours match.
- **Missing**: gutter fg/bg tokens; a gutter rule-off switch; an active-line token (or none); an
  editor font size and line height; a toolbar surface.

### 2.7 Buttons: every variant the frame draws

All of them are 26 high, padding 5/10, radius 7, 12 px Regular, with **no border**. The frame
draws one size and only the rest state.

| variant | node | Figma | purlis nearest | gap |
| --- | --- | --- | --- | --- |
| ghost | `6:120`, `6:122`, `6:124` | no fill, `#a09eab` text | `.ui-button-ghost`: transparent, `text.primary`, 14 px (`controls.css:70-74`) | text colour (`text.primary` against `#a09eab`), size 14 against 12 |
| accent (soft primary) | `6:126` "Ask a chat" | lavender 0.12, `#c3baf5` text | `.ui-button-primary`: `accent.surface` + **`accent.base` 1 px border** + `text.primary` (`controls.css:60-64`) | border, text colour |
| filled | `6:128` "Add to context" | `#2a2a31`, `#e6e4ec` | `.ui-button`: `control.base` `#24242a` + 1 px `border.strong` `#55535f` (`controls.css:36-50`) | fill, border |
| filled on card | `6:512`, `6:520` "Got it" | `#2f2f37`, `#e6e4ec` | `.ui-button` | **no token** for a button on a card |
| link / text | `6:514`, `6:522` | no fill, `#b3a8f2` | none in the UI (`markdown.link` only) | **no variant** |
| warning soft | `6:528` "Update layout" | amber 0.14, `#e5c08a` | none (`.ui-button-danger` is the only toned variant, `controls.css:82`) | **no variant**, no wash token |
| icon `+` | `6:34`, `6:109` | 28 × 28, radius 8, 16 px `#a09eab` | — | radius 8 missing |

- **Expressible**: no.
- **Missing** (as component keys): `button.border` (or a width), `button.font-size`,
  `button.padding-x`, per-variant fg/bg (`ghost.fg`, `primary.fg`, `primary.border`), new
  variants (link, warning-soft, on-card), and per-control radius.

### 2.8 Inputs (`6:70` "Filter chats…", `6:556` "+ Add a todo…")

- **Anatomy**: `#17171b` fill, no border, radius 8, padding 8/10, 13 px, placeholder `#6f6d7a`.
- **Purlis**: `.ui-input` has `surface.sunken` and a 1 px `border.strong` (`#55535f`), with
  `radius.md` 7, `size.control` height, `font-size.base` 14 and `placeholder` `#8a8896`
  (`controls.css:167-180`).
- **Expressible**: the fill only.
- **Missing**: input border-off or width; input radius (8); input font size; placeholder at
  `#6f6d7a` (blocked by the floor at `check.rs:597`).

### 2.9 Selects, switches, radios, dialogs, menus, tooltips

**The frame draws none of these.** Purlis styles them: `.ui-select` (`controls.css:167-230`),
`.ui-switch` (`:351-397`), `.ui-radio` (`:289-345`), and `.ui-dialog`/`.ui-popover`/`.ui-menu`/`.ui-tooltip`
(`overlays.css:24-60`, all `surface.overlay`, `border.subtle`, `radius.lg`, `elevation.overlay`).
**There is no Figma source to match them to.** If they are to follow Figma, the design has to
draw them. If not, extending the frame's language to them (no border, radius 7–10, washes) is
a design call, not a sync.

### 2.10 Segmented / scope switch (`6:63`)

- **Anatomy**: the track is `#17171b`, padding 2, radius 7, no border. The chosen segment is
  `#2a2a31`, radius 5, padding 3/8, 11 px `#e6e4ec`. Unchosen segments have no fill and 11 px
  `#8a8896` text.
- **Purlis**: `.ui-levels` has a 1 px `border.subtle` and `radius.lg` 10. Its segments are
  `control.base` fills joined by `border.subtle` rules, with `text.secondary` text, and the
  chosen one is `accent.surface` (`App.css:7813-7857`). `.ui-segmented` is the same with
  `border.strong` (`controls.css:405-440`).
- **Expressible**: no.
- **Missing**: track fill, track inset, segment radius (5), chosen-segment fill (neutral, not
  accent), and segment rules off.

### 2.11 Filter chips (`6:74`, `6:76`) and badges (`6:29`, `6:546`)

- **Chips**: `#24242a`, radius 999, padding 4/10, 12 px `#a09eab`.
- **Badges**: radius 999, padding 1/7–8, 11 px `#8a8896`. The workspace badge is on `#2a2a31`
  and the repo badge on `#24242a`.
- **Purlis**: `.count-pill` is `control.count` with `text.secondary` and `font-size.xs`
  (`App.css:590-605`). Chips are buttons, so they carry the border issue in 2.7.
- **Missing**: badge fg (quiet tier); a second badge fill; chip radius full on a button.

### 2.12 Lists (Repos `6:538`–`6:549`, Todos `6:560`–`6:583`, Notices cards `6:507`–`6:529`)

- **Repo row**: chosen = lavender 0.10, radius 8, padding 8/10, name Medium 13 + note 12
  `#8a8896`. Children are indented 24 and the empty line 36.
- **Todo row**: a 14 px round checkbox with a 1.5 px `#55535f` ring, gap 10, 13 px `#c9c7d2`.
  The group label "Earlier" is 11 px `#6f6d7a`.
- **Notice card**: `#24242a` (the amber card is amber 0.07), radius 10, padding 12, gap 10. The
  sentence is 13/1.55 `#c9c7d2` with an `#8a8896` detail, or an amber Medium lead word. Actions
  are a filled-on-card button plus a link button.
- **Purlis**: `.ui-row` is square (`collections.css:195-215`). Selected adds `list.selected`
  plus a 2 px edge (`:248-262`). `.notice-list` is one bordered `surface.raised` box with
  `radius.md` (`App.css:2897-2902`), not separate cards.
- **Missing**: row radius; a selected-edge-off switch; a card fill or per-card radius (10);
  checkbox size, round shape and ring width; a waiting-wash card; link button; quiet tiers.

### 2.13 Status bar (`6:609`)

- **Anatomy**: `#141417`, 30 high, padding 0/16, gap 14, no top border. Items are 12 px: the
  workspace in `#c9c7d2` with an accent dot, the rest in `#8a8896`, "6 warnings" in amber, and
  the folder in Mono 12 `#6f6d7a`.
- **Purlis**: `.status-line` is `surface.deep`, with a `border.subtle` top, `space-5` gap and
  `space-2`/`space-4` padding (`App.css:5286-5298`).
- **Expressible**: colour yes; border, height and gap no.
- **Missing**: status bar height (30), padding 16 and gap 14 (off scale), a border-off switch,
  and the quiet tiers.

---

## 3. What Figma has that purlis has no surface for at all

1. **A window inset and panel gutters.** The window's 12 px rounded corner shows `#121215`
   under it, and the regions float with 8 px gaps. Purlis has `window.background` but lays its
   regions out flush, separated by hairlines. This is a layout mode, not a token.
2. **A plane-name title bar.** The top row is the plane name, an unsaved dot and pills. Purlis's
   top row is the projects strip (`App.css:5815`).
3. **Pills as a component**: rounded status, ghost and accent pills in the chrome (`6:16`,
   `6:19`, `6:21`). Purlis has no pill button. Its chrome buttons are `radius.md` boxes.
4. **Toned washes**: amber at 0.07, 0.10 and 0.14, and lavender at 0.10, 0.12 and 0.14 as fills
   for a card, a pill, a button and a rail button. Purlis has no wash tokens except `danger.wash`
   and `diff.*`.
5. **Link / text buttons** in the UI (`6:514`).
6. **Separate notice cards** with their own actions in a column (`6:501`). Purlis draws one
   bordered list.
7. **Six text tiers.** Purlis has three plus a placeholder, and the two quietest Figma tiers are
   below purlis's text floor.
8. **A borderless control language.** No control, strip, rail or panel has a hairline. Purlis
   has no border-width or border-off token anywhere.
9. **Content-sized, top-rounded file tabs** that join the toolbar, and **chip-shaped workspace
   tabs**. Purlis's equal-share, square, divided strips are a structural rule
   (`App.css:360-404`, `fits.ts`), not a theme value.
10. **The editor toolbar as a surface**, holding file name, path and actions (`6:112`).

The design has no states, no light appearance and no high-contrast appearance. The Purlis
theme's light and high-contrast appearances are derived by rule (`docs/design-system.md:391-398`),
so they have no Figma source.

## 4. What it would take, in order

1. **Component tokens** as ADR 0097 §1 already decided, but not yet built. Add them to
   `vocabulary.rs` and alias them in `controls.css`, `collections.css` and `overlays.css`, with
   at least these keys:
   - per control: fg, bg, border colour, border width (allowing 0) and radius;
   - button: per-variant fg/bg/border;
   - tab: corner radius per corner, gap and sizing;
   - rail: button size, radius, chosen fill and chosen fg;
   - panel: radius, gutter and padding;
   - gutter: fg and bg.
2. **New semantic roles**:
   - text tiers: `text.quiet` (`#8a8896`) and `text.faint` (`#6f6d7a`), each with its own floor;
   - `accent.text` (`#c3baf5`) and `link` (`#b3a8f2`);
   - washes: `accent.wash.{weak,base,strong}` and `waiting.wash.{weak,base,strong}`, as alpha
     laid on the ground, not flattened;
   - surfaces: `surface.rail` / `surface.titlebar` apart from `surface.deep`;
   - fills: `control.on-card` (`#2f2f37`) and `editor.toolbar`;
   - the editor: `editor.gutter`, `editor.font-size` and `editor.line-height`.
3. **Bounds**:
   - `radius.*` ranges, because their caps stop nothing Figma needs except via role;
   - a 12.5 step or a fractional `font-size.*`;
   - off-scale spacing, needed for 10 and 14, which the frame uses everywhere;
   - chrome heights 44, 40 and 30.
4. **A layout mode** for floating panels and content-sized tabs. This is an app change
   (`App.css` and `fits.ts`), not a theme value.
5. **The built-in "no warnings" test** (`purlis_tests.rs`, `docs/design-system.md:370`) has to
   allow the quiet tiers, or the tiers get their own floors. This calls for an operator ruling
   on contrast against Figma.
