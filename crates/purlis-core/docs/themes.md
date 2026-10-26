# Themes

A **theme** is how the window looks: every colour, every motion value and its shape and size,
in each of its appearances (dark, light, high contrast). purlis's theme engine reads each theme,
refuses one that breaks its grammar, decides which theme the window draws, and checks whether
its words can be read. The window only draws what the engine hands it. The full format is in the
repository's `docs/design-system.md`, and its schema is `docs/schema/theme.v2.schema.json`.

## Where a theme comes from

- **Built in**: Purlis, `purlis`, the default, with a dark, a light and a high-contrast dark
  appearance; and Purlis Classic, `purlis-classic`, the alternative, with a dark and a light
  appearance. Where nothing chose a theme, Purlis is drawn. `purlis-dark` and `purlis-light`
  (and `charter-dark`, `charter-light`) name Purlis's dark and light, as `system` follows the
  system with Purlis; only `purlis-classic` names Classic.
- **Your own**, at the machine level: `theme.json` in purlis's config folder
  (`$PURLIS_CONFIG_HOME`, else `$XDG_CONFIG_HOME`, else `~/.config`, then `purlis/`), whose id
  is `mine`, and each theme folder in that folder's `themes/`, whose id is `machine:<folder>`.
- **The project's**: each theme folder in `themes/` at the plane root, `project:<folder>`.
- **A workspace's**: each theme folder in `workspaces/<ws>/themes/`,
  `workspace:<ws>:<folder>`.
- **An extension**: each theme an approved extension contributes while it is on. Its id is
  `<extension>/<theme>`, as a project's pick names it.

A theme folder's id names its level because a folder of the same name can be at several levels;
a pick names it by its folder's name alone, and the nearest level within the pick's reach is
read. The theme commands read the levels' folders exactly as the window does, with the same
reach: a folder extends the folders of its own level and the levels farther out. Outside a
plane there are no project or workspace levels. What reading a level has to say (a `themes/` reached through a link, more folders than are
read) is attached to every theme at that level: as a note on a theme that is drawn, among the
reasons on one that is refused.

`purlis theme list` shows every theme available here, the machine's first, then the project's,
each workspace's by name, and the extensions':

```
  ID                        NAME            APPEARANCES  FROM
  purlis-classic            Purlis Classic  dark, light  built-in
  mine                      Late            dark         machine /home/me/.config/purlis/theme.json
  machine:dusk              Dusk            dark, light  machine /home/me/.config/purlis/themes/dusk
  project:team              Team            dark         project /work/plane/themes/team
  workspace:ops:alarm       Alarm           dark         workspace ops /work/plane/workspaces/ops/themes/alarm
  solarized/Solarized Dark  Solarized Dark  dark         extension solarized
```

FROM starts with the theme's level: `built-in`, `machine`, `project`, `workspace <ws>` or
`extension <extension>`.

APPEARANCES names each appearance by its id, with its tag when the id is not one (`day
(light)`) and `high contrast` when it is tagged so. An id may hold a space, as an extension's
theme name can: quote it on the command line (`purlis theme check "solarized/Solarized Dark"`).

Then `refused:`, one line per theme purlis refused and why. `--json` answers
`{"schema": 1, "theme_schema", "themes": [{"id", "name", "appearances", "options",
"combinations", "from", "refused"}]}`, where `theme_schema` is the address of the theme schema
this purlis reads (see [The schema a theme names](#the-schema-a-theme-names)), each appearance is `{"id", "appearance", "high_contrast"}`, each option is
`{"id", "label", "description", "kind", "default", "values": [{"id", "label"}]}` (a toggle's
values are `off` and `on`, so its `default` here is `"on"` or `"off"`, where `theme.json`
writes `true` or `false`), `combinations` is how many appearance × option combinations the
theme has, `from` is `{"kind": "built-in"}`,
`{"kind": "mine", "path"}`, `{"kind": "machine", "folder", "path"}`,
`{"kind": "project", "folder", "path"}`, `{"kind": "workspace", "workspace", "folder", "path"}`
or `{"kind": "extension", "extension"}`, `level` is `built-in`, `machine`, `project`,
`workspace` or `extension` (`null` for a theme named by its path), and `refused` is `null` or
why (`name`, `appearances`, `options` and `combinations` are then `null`).

**Each level's choice** follows the themes, nearest first, when any level chose: the person's
choice and the committed default of each workspace, by name; then the project's; then the
person's machine default (outside a plane, that alone):

```
chosen, nearest first:
  CHOSEN BY               THEME           FROM     APPEARANCE  OPTIONS
  workspace alpha, yours  dusk            project  dark        borders=on
  project, yours          dusk            project  -           accent=lavender
  project, committed      dusk            project  -           accent=teal
  machine, yours          purlis-classic  -        light       -
```

`yours` is the person's choice on this machine; `committed` the default the project's manifest
or a LIVE workspace's `workspace.json` commits. FROM is the level of the theme folder a choice
reads (`-` for a theme that is no folder). In a level, the person's choice beats the committed
default, and its option values are its own. `--json` has them as `choices`:
`[{"level", "workspace", "theme", "id", "from", "appearance", "options"}]`, where `level` is
`workspace-choice`, `workspace-default`, `project-choice`, `project-default` or
`machine-default`, `workspace` names the workspace of the first two (`null` otherwise), `theme`
is the pick as written, `id` is the id this list names its theme by (`null` for a folder no
level within the choice's reach has), `from` is `workspace`, `project`, `machine` or `null`
(the level the choice reads now, placed as the engine places it: a choice stored with no
`from` shows the level it reads, not what `theme-choices.json` holds),
`appearance` is `null` when none was chosen, and `options` is the value of each option by its id
(`{}` for none). Whether a choice is drawn where it is made also hangs on the extensions in force
there and the system's appearance; Settings › Theme says what is drawn.

## Theme options

A theme may declare **options**: toggles and choices a person sets, each value a layer of token
values drawn over every appearance, and over one appearance by its id. There are no
free-number options.

```json
"options": [
  { "id": "borders", "kind": "toggle", "label": "Borders", "default": false,
    "on": { "tokens": { "border.subtle": "rule" } } },
  { "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
    "values": [
      { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
      { "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" },
        "appearances": { "day": { "tokens": { "accent.base": "#0f6e64" } } } }
    ] }
]
```

An option nothing chose draws its default. The person chooses values in Settings › Theme, and a
project or a LIVE workspace may commit them with its default theme, by each option's id:
`[theme] options = { accent = "teal" }` in `charter.toml`, `settings.theme.options` in a
`workspace.json`. The check covers **every combination** of
appearance and option values, so a theme has at most 64: the appearances times each option's
number of values (a toggle has two). A theme past that is refused with its count.

## Geometry

A theme sets its shape and size as numbers: the spacing scale, the type scale, the weights, the
line heights, the corners, the control and strip heights, the focus ring and the shadows. Each
is a **geometry token** with a bound; a value outside it, a fraction where a whole number is due
(every one but the line heights), or anything that is not a number refuses the theme with a
sentence that names the bound. The engine writes what the window draws from the number. A
theme names only what it changes; every other token keeps the value of the theme it extends,
and Purlis Classic's when it extends none.

**Geometry is set per appearance**, in each appearance's `tokens` beside its colours:

```json
"appearances": [
  { "id": "dark", "appearance": "dark",
    "tokens": { "radius.md": 7, "radius.lg": 10, "size.control": 26, "line-height.body": 1.55 } },
  { "id": "light", "appearance": "light",
    "tokens": { "radius.md": 7, "radius.lg": 10, "size.control": 26, "line-height.body": 1.55 } }
]
```

An appearance that leaves a token out keeps the value of the appearance of the same id in the
theme it extends, else Purlis Classic's. So a theme that rounds its corners in `dark` alone
draws `light` as the theme it extends does, square if it extends Classic: name the tokens in
every appearance. A theme option's value may set geometry too, within the same bounds, drawn
over the appearance as its colours are.

| Token | Unit | Bounds | Classic | Compact | What it reaches |
|---|---|---|---|---|---|
| `space.0` | px at 14px | 0–0 | 0 | 0 | no space: 0 in every theme |
| `space.1` | px at 14px | 0–4 | 2 | 2 | padding, gaps and margins: the smallest step |
| `space.2` | px at 14px | 1–8 | 4 | 3 | padding, gaps and margins |
| `space.3` | px at 14px | 2–10 | 6 | 4 | padding, gaps and margins |
| `space.4` | px at 14px | 4–12 | 8 | 6 | a control's side padding; padding, gaps and margins |
| `space.5` | px at 14px | 6–16 | 12 | 8 | padding, gaps and margins |
| `space.6` | px at 14px | 8–24 | 16 | 12 | padding, gaps and margins |
| `space.7` | px at 14px | 12–32 | 24 | 16 | padding, gaps and margins |
| `space.8` | px at 14px | 16–48 | 32 | 24 | the largest padding and margins, and the width of menus and tooltips (a share of it) |
| `font-size.2xs` | px at 14px | 8–12 | 10 | 10 | the small levels control |
| `font-size.xs` | px at 14px | 9–13 | 11 | 11 | the meta and label roles: badges, counts |
| `font-size.sm` | px at 14px | 10–14 | 12 | 12 | the secondary role: secondary lines, help text |
| `font-size.md` | px at 14px | 11–15 | 13 | 13 | the body role: a tab's title, dense lists, a menu's rows |
| `font-size.base` | px at 14px | 12–18 | 14 | 14 | fields', buttons' and segmented controls' words, the heading role |
| `font-size.lg` | px at 14px | 13–22 | 16 | 16 | the title role |
| `font-size.xl` | px at 14px | 16–28 | 20 | 20 | the Welcome page's heading and its doors' marks |
| `font-weight.regular` | weight | 300–500 | 400 | 400 | the body, secondary, meta and code roles, menu rows |
| `font-weight.medium` | weight | 400–600 | 500 | 500 | a form's label: a setting's name, a choice's name |
| `font-weight.strong` | weight | 500–800 | 600 | 600 | titles, headings, labels, a row's strong line |
| `line-height.tight` | × text | 1–1.4 | 1.2 | 1.2 | a compact line: counts |
| `line-height.body` | × text | 1.2–1.8 | 1.4 | 1.3 | the window's own line, and every line the fit checks hold |
| `radius.sm` | px | 0–8 | 3 | 3 | labels, chips, crumbs, badges, checkboxes, error and report boxes |
| `radius.md` | px | 0–10 | 4 | 4 | menu rows, Notices, the palette's rows, Inbox asks, a tab's hover pill and its `×` |
| `radius.control` | px | 0–14 | 4 | 4 | an md control's corner: buttons, fields, selects, a list's rows, title-bar chips, a tab's top (#3113) |
| `radius.lg` | px | 0–14 | 6 | 6 | dialogs, popovers, menus, tooltips, segmented controls |
| `radius.xl` | px | 0–18 | 8 | 8 | the Welcome page's doors, nothing else |
| `radius.full` | px | 0–9999 | 9999 | 9999 | pills: counts, toggle tracks |
| `size.control` | px at 14px | 20–40 | 28 | 24 | buttons, fields, selects, menu rows, a list's rows, the workspace strip |
| `size.chrome` | px at 14px | 16–32 | 21 | 18 | small buttons, the chat strip, pane and panel headers, a dense list's rows and every tree row |
| `window.gutter` | px | 0–16 | 0 | 0 | the gap between the window's regions, and around them inside the frame |
| `panel.corner-radius` | px | 0–24 | 0 | 0 | a region panel's corners: the sides, the content and the bottom panel's halves |
| `panel.border-width` | px | 0–2 | 1 | 1 | the hairline between two regions, drawn only while they sit flush (no gutter) |
| `focus-ring.width` | px | 1–4 | 2 | 2 | every focus ring, and the selected row's edge |
| `focus-ring.offset` | px | 0–4 | 2 | 2 | a ring outside its element: a control's, a card's |
| `focus-ring.inset` | px | −4–0 | −2 | −2 | a ring inside its element: a list row's, a tab's, a segment's |
| `pane.border-width` | px | 0–2 | 1 | 1 | the line around a pane, the focused one's in the accent; Purlis draws none (#2914) |
| `elevation.raised-y` | px at 14px | 0–8 | 2 | 2 | how far a raised bar's shadow falls: the find bar |
| `elevation.raised-blur` | px at 14px | 0–24 | 8 | 8 | how soft that shadow is |
| `elevation.overlay-y` | px at 14px | 0–16 | 8 | 8 | how far an overlay's shadow falls: dialogs, popovers, menus, tooltips |
| `elevation.overlay-blur` | px at 14px | 0–48 | 24 | 24 | how soft that shadow is |

**The units.** *px at 14px* is a length in px at the window's default text size, written in rem,
so it grows with the person's text-size setting. *px* is a length that does not grow: corners
and the focus ring. *weight* is a font weight. *× text* is a line height, a share of the text's
own size, and the only kind that takes a fraction. **Classic** is Purlis Classic's comfortable
value, which a theme that extends nothing draws unless it names the token; **Compact** is
Purlis Classic's compact value. The window's own text size is the person's text-size setting,
not a token: words with no size of their own (a list row's, a paragraph's) follow it, and every
*px at 14px* length grows with it.

The project strip sits in the title bar, whose height no token reaches yet. **Still square,
whatever `radius.*` says:** the project and workspace strip tabs, the settings gear on a strip,
and every tree row. The session tabs and their `×` are rounded through their parts ([Session
tabs](#session-tabs)), and a list's rows by `radius.control` (the list family, since manifest
version 11; `radius.md` before #3177). **The focus ring tokens reach every focus ring**, the strip tabs', a tab's `×` closer's and
the strip gear's among them; a ring that sits half as far out or in draws half the offset. Under
forced colours the ring is drawn in the system's `Highlight` at the theme's width and offset:
forced colours replace colour, not geometry.

### Density's compact layer

Density is the person's setting, comfortable or compact, and every theme has both. A theme's
top-level `compact` object holds what compact draws, for every appearance: geometry tokens and
their numbers, held to the same bounds. It is layered over the `compact` of the theme it
extends.

```json
"compact": { "size.control": 22, "radius.md": 5 }
```

What compact draws for each token (`compact_of` in the engine):

- a token the `compact` layer names is drawn as it names it;
- a token Purlis Classic's compact moves (its Compact differs from its Classic above:
  `space.2` to `space.8`, `line-height.body`, `size.control`, `size.chrome`) is the smaller of
  Classic's Compact and the appearance's own value, so compact is never looser than
  comfortable;
- every other token keeps the appearance's own value.

So a theme with `radius.md` 7 keeps 7 in compact, and one with `size.control` 26 draws 24 there.

### A line fits its box

A theme can set each number inside its bound and still make a line taller than the box it is
drawn in. The check holds two lines, in both densities:

- a field's words: `font-size.base` × `line-height.body` within `size.control`;
- a strip's tab title: `font-size.md` × `line-height.body` within `size.chrome`.

Purlis Classic fits each with room:

- comfortable: 14 × 1.4 = 19.6 within 28, and 13 × 1.4 = 18.2 within 21;
- compact: 14 × 1.3 = 18.2 within 24, and 13 × 1.3 = 16.9 within 18.

Compact is the tighter: `font-size.md` 14 fits comfortable (14 × 1.4 = 19.6, within 21) and not
compact (14 × 1.3 = 18.2, over 18). A line that does not fit is a `line-does-not-fit` warning that
names its density (see [The theme check](#the-theme-check)); the theme is drawn. A button's label
sits on a line of 1, so it fits wherever a field's words do. Every length here grows with the text
size alike, so what fits at the default text size fits at every one.

## Component parts

A token reaches every control that draws from it. A **component part** reaches one named part of
one component family: the button's root, its label, its icon; a menu's row and its words, its key
and its tick; the surface a popover and a menu stand on, the rule between a menu's groups, a
popover's filter row, list and footer. A theme sets parts in a top-level `parts` object of
`theme.json`, for every appearance, beside its tokens:

```json
"parts": {
  "button.root.corner-radius": "full",
  "button.root.border-width": 0,
  "button.root.primary.hover.fill": "accent.base",
  "button.label.font-weight": 500,
  "menu.root.hover.fill": "list.selected",
  "popover.root.menu.corner-radius": 10
}
```

`purlis theme parts` prints every family, part, variant, state and property there is, with each
bound, as JSON: what the theme skill, the gallery and an editor read. The schema completes each
key. A theme that extends another starts from its parent's parts and layers its own over them,
key by key. A property typed in its CSS spelling (`border-color`) is told the name it means.

**The manifest's version is a release's.** Its `version` is the last version a release shipped,
and it grows once per release that adds a family, a part, a variant, a state or a property, never
once per change (ADR 0103, amended 2026-10-25). What has been added since the last release is
listed with `since` one above `version`: a dev build can carry such additions, and a stable
release gives them that version. The schema says the same: `manifest version <VERSION>, plus
unreleased additions marked since <NEXT>`, where NEXT is VERSION + 1. The manifest never takes
anything away, and what the last release shipped never changes: a test holds the manifest as of
the last released version to that version's digest, taken over its contract (the families,
parts, variants, states and properties, their bounds and `since`), so a reworded description is
no change to it.

**The key** is `<family>.<part>[.<variant>][.<state>].<property>`. The state at rest is implied: a
key with no state is the part at rest, and in every state that does not set the property. A key
with no variant reaches the part in every variant; one with a variant only that variant. The more
a key names, the more it wins: a state over rest, then a variant over every variant. A key that
names a family, part, variant, state or property purlis does not have is said and left out, as
an unknown token is.

One row a family, in the families' names' order, written from the manifest (`purlis theme parts`
says what each family, part and variant is):

| Family | Parts | Variants | States |
|---|---|---|---|
| `activity` (since version 10) | `item` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`), `icon` (`text`), `count` (`fill`, `text`), `indicator` (`fill`) | `tab`, `bar` | `hover`, `pressed`, `selected`, `focused` |
| `button` | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `label` (`text`, `font-size`, `font-weight`), `icon` (`text`) | `secondary`, `primary`, `ghost`, `danger`, `grants`, `soft-accent` (since version 2), `warning-soft` (since version 2), `link` (since version 2), `quiet` (since the next release) | `hover`, `pressed`, `selected`, `focused`, `disabled`, `open` |
| `checkbox` (since the next release) | `root` (`fill`, `text`, `corner-radius`, `gap`), `box` (`fill`, `border-colour`, `border-width`, `corner-radius`), `label` (`text`, `font-size`, `font-weight`), `description` (`text`, `font-size`) | `box`, `chip` | `hover`, `selected`, `focused`, `disabled` |
| `combobox` (since the next release) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `height`), `chevron` (`text`) | none | `hover`, `focused`, `disabled`, `open`, `invalid` |
| `dialog` (since version 7) | `root` (`fill`, `border-colour`, `border-width`, `corner-radius`, `shadow`), `title` (`text`, `font-size`, `font-weight`), `subtitle` (`text`), `description` (`text`), `separator` (`fill`), `summary` (`text`) | `dialog`, `alert` | `hover`, `pressed`, `selected`, `focused`, `disabled`, `open` |
| `editor` (since version 8) | `toolbar` (`fill`, `text`, `padding`, `gap`, `font-size`) | `default` | none |
| `empty-state` (since version 4) | `root` (`padding`, `gap`), `icon` (`text`), `headline` (`text`, `font-size`, `font-weight`), `body` (`text`, `font-size`), `action` (`gap`) | `page`, `panel` | none |
| `field` (since version 3) | `root` (`gap`), `label` (`text`, `font-size`, `font-weight`), `help` (`text`, `font-size`), `error` (`text`, `font-size`) | none | `disabled` |
| `list` (since version 11) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `body` (`padding`, `gap`), `lead` (`text`, `gap`), `label` (`text`, `font-size`, `font-weight`), `description` (`text`, `font-size`), `trail` (`text`, `gap`), `header` (`text`, `font-size`, `font-weight`), `shortcut` (`text`, `font-size`) (since the next release) | none | `hover`, `focused`, `selected`, `disabled` |
| `menu` (since version 6) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `label` (`text`, `font-size`, `font-weight`), `description` (`text`, `font-size`), `shortcut` (`text`, `font-size`), `indicator` (`text`) | `default`, `danger` | `hover`, `focused`, `selected`, `disabled`, `open` |
| `option-card` (since the next release) | `root` (`fill`, `text`, `border-colour`, `shadow`, `corner-radius`, `padding`, `gap`), `icon` (`text`), `title` (`text`, `font-size`, `font-weight`), `detail` (`text`, `font-size`), `check` (`text`) | `card`, `tile` | `hover`, `selected`, `focused`, `disabled` |
| `panel` (since version 4) | `root` (`fill`, `border-colour`, `border-width`, `corner-radius`, `shadow`) | `side`, `content`, `bottom` | none |
| `panel-section` (since version 4) | `root` (`padding`, `gap`), `head` (`padding`, `gap`), `title` (`text`, `font-size`, `font-weight`) | `default` | none |
| `panel-separator` (since the next release) | `root` (`fill`), `grip` (`fill`) | none | `hover`, `pressed`, `focused` |
| `popover` (since version 6) | `root` (`fill`, `text`, `border-colour`, `border-width`, `corner-radius`, `shadow`), `filter` (`fill`, `padding`), `rule` (`fill`), `list` (`padding`, `gap`), `header` (`text`, `font-size`, `font-weight`), `footer` (`fill`, `text`, `padding`, `font-size`), `key` (`fill`, `text`, `corner-radius`) | `popover`, `menu` | none |
| `radio` (since the next release) | `root` (`text`, `gap`), `mark` (`fill`, `border-colour`, `border-width`), `label` (`text`, `font-size`, `font-weight`), `description` (`text`, `font-size`) | none | `hover`, `selected`, `focused`, `disabled` |
| `rail` (since version 9) | `button` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `monogram` (`font-size`, `font-weight`), `badge` (`fill`, `text`), `unsaved` (`fill`) | `project`, `action` | `hover`, `selected`, `focused` |
| `segment` (since the next release) | `root` (`fill`, `text`, `shadow`, `corner-radius`), `label` (`text`), `count` (`text`) | none | `hover`, `selected`, `focused`, `disabled` |
| `segmented-control` (since the next release) | `root` (`fill`, `corner-radius`, `shadow`, `padding`, `gap`) | none | none |
| `select` (since the next release) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `height`), `placeholder` (`text`), `chevron` (`text`) | none | `hover`, `focused`, `disabled`, `open`, `invalid` |
| `slider` (since the next release) | `root` (`text`, `gap`), `track` (`fill`, `corner-radius`), `fill` (`fill`, `corner-radius`), `thumb` (`fill`, `border-colour`, `corner-radius`), `output` (`text`) | none | `disabled` |
| `status-item` (since version 4) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `icon` (`text`), `label` (`text`, `font-size`, `font-weight`), `count` (`text`) | `bar`, `pill`, `waiting`, `accent` | `hover`, `pressed`, `focused`, `disabled` |
| `switch` (since the next release) | `root` (`text`, `gap`), `track` (`fill`, `corner-radius`), `thumb` (`fill`, `corner-radius`), `label` (`text`, `font-size`, `font-weight`) | none | `hover`, `selected`, `focused`, `disabled` |
| `tabs` (since version 8) | `tab` (`fill`, `text`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`, `max-width`, `flare`, `inset`), `label` (`text`, `font-size`, `font-weight`), `closer` (`fill`, `text`, `corner-radius`), `count` (`fill`, `text`, `corner-radius`), `indicator` (`fill`, `height`, `corner-radius`, `position`), `strip` (`fill`, `border-colour`, `border-width`), `layout` (`attach`, `sizing`, `dividers`, `gap`) | `chat`, `view`, `file`, `background` | `hover`, `selected` |
| `text-input` (since version 3) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `height`), `icon` (`text`) | none | `hover`, `focused`, `disabled` |
| `textarea` (since version 3) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `font-size`) | none | `hover`, `focused`, `disabled` |
| `titlebar` (since version 12) | `workspace` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`, `padding`, `gap`, `font-size`, `font-weight`, `height`), `count` (`fill`, `text`), `crumb` (`fill`, `text`, `corner-radius`), `all` (`fill`, `text`, `corner-radius`), `pin` (`fill`, `text`, `corner-radius`), `row` (`fill`, `text`, `corner-radius`) | `pinned`, `unpinned` | `hover`, `selected`, `focused` |
| `tooltip` (since version 5) | `root` (`fill`, `text`, `border-colour`, `shadow`, `border-width`, `corner-radius`), `detail` (`text`), `shortcut` (`text`, `font-size`) (since the next release) | none | none |

A field is its label, its control (a `text-input` or a `textarea`), its help and its error, as
one: `field.help.text` reaches a field's help line, `text-input.root.border-width` every one-line
box. A family with no variants is named by its part alone (`text-input.root.fill`); its words are
measured on its own fill as a button's are, and its floors fall back to its own default, a field's
help to the quiet tier and its error to the danger colour.

A dialog's size, height, place and rows are the dialog rules', never a theme's: a dialog key for
its height or its padding is said and left out.

A select's and a combobox's box are drawn as the text input's, so `select.root.border-width` and
`text-input.root.border-width` together reach every box of a form. Their lists are no part of
their own: a select's list and a combobox's suggestions stand on the popover's `menu` surface and
each option is a menu's row, so `menu.root.hover.fill` reaches an option too. A chevron is a mark,
held at 3:1 as an icon is; the placeholder and the chevron fall back to the quiet tier.

A choice is chosen in `selected`: a box ticked (`checkbox.box.selected.fill`), a radio, a segment or
a card picked, a switch on (`switch.track.selected.fill`). A choice's size is its family's: no
choice takes a height, and a mark's corner is clamped to half its own box, so a tick never moves
anything.

The states are `hover`, `pressed`, `selected` (chosen, or a toggle that is on), `focused` (the
keyboard is on it; a field whenever it holds the caret), `disabled` and `open`. A family names
only the states its wrapper marks: the button every one; a text input and a textarea `hover`,
`focused` and `disabled`; a select and a combobox `hover`, `focused` (a select from the keyboard, a
combobox whenever it holds the caret), `disabled` and `open`; a field's root `disabled` alone; a
dialog every one, though none changes its size; a choice `hover`, `selected`, `focused` and
`disabled`, and a slider's row `disabled` alone (its thumb, not its row, is under the pointer and
the keyboard). A key naming another is said and left
out.

| Property | Value | Bounds |
|---|---|---|
| `fill` | a semantic colour token, or `transparent` | |
| `text` | a semantic colour token | |
| `border-colour` | a semantic colour token, or `transparent` | |
| `shadow` | `none`, `raised` or `overlay` | |
| `border-width` | px | 0–4 |
| `corner-radius` | one value or four (top left, top right, bottom right, bottom left): px, a `radius.*` token or `full` | 0–9999, drawn at most half the part's height; `full` is a pill at any height |
| `padding` | one value, or two (block, inline): px at 14px or a `space.*` token | block 0–12, inline 0–32 |
| `gap` | px at 14px or a `space.*` token | 0–16 |
| `font-size` | px at 14px or a `font-size.*` token | 10–18 |
| `font-weight` | a weight or a `font-weight.*` token | 300–700 |
| `height` | px at 14px or a `size.*` token | 24–48 |

**A colour names a semantic token**, never a hex or a palette name, so the palette, the
appearances, the tints and the floors keep working for a part as they do for a token. A geometry
token follows the person's density; a number is the same in both.

**A state never changes size.** Inside a state only `fill`, `text`, `border-colour` and `shadow`
may be set, so nothing moves when the pointer, a press, the keyboard or a choice reaches a part.
A theme that sets any other property inside a state is refused, and the check says why.

**A corner is clamped as it is drawn** to half a control's height, its variant's own height
where the theme sets one, and `full` draws a pill at any height and density. A dialog's corner is
a surface's: it is drawn at most `space.6`, the inset of what the dialog holds, and `full` is
that, so a rounded dialog never cuts off a word or an answer in a corner. **A control stays a target and its words fit**: in both densities a button
the theme sizes is at least 24px tall, and its words (its font size on a line of 1), its block
padding and its border, top and bottom, fit in its height. A theme that breaks either is refused
with the density named: Purlis Classic's compact button is 24px, so 14px words and a 1px border
leave room for 4px of padding above and below, not 6.

**What a theme does not set is the family default**, which the window's own stylesheet draws from
the global tokens. A theme that names no parts draws exactly as it did; the engine writes the
parts a theme names as one stylesheet, keyed on the wrapper's `data-scope` and `data-part`, its
`data-variant` and its state attribute, in the `parts` layer above the controls.

A geometry token is held to the property's bound at the value the theme draws it at, in both
densities: `"gap": "space.8"` is 32 comfortable, past a gap's 16, and refuses the theme.

**The floors hold per part and per state.** In each variant and state a theme's parts change,
the part's words contrast 4.5:1 with its fill, and its icon 3:1, on every surface the family is
drawn on, each part's words in the tier they are drawn in (a dialog's subtitle muted, its
question secondary and its summary quiet); a border that is the part's only edge, on a fill that is its ground or none, contrasts
3:1. A part that breaks one draws its family default in that variant and state, and the check
says each as a `part-falls-back` warning. A disabled control is exempt, as WCAG has it.

**A family's grounds are every surface the window draws it on**: for a button, the window's
surfaces and the strips' layers, and a Notice's danger surface and wash among them. A wash is
read laid over the surface under it. `app/src/theme/grounds.test.ts` finds them from the components, and fails until the
manifest lists a new one.

**States that hold at once are held as one.** A button can be hovered and pressed, or focused and
selected, at the same time, and the stylesheet's order decides which state's colour each property
takes. Every such set is resolved that way and held to the same floors, the consent floors too.
A part that breaks one in a set draws its family default in the state the stylesheet writes
last, and the warning names the set (`button.root.primary.hover+focused`).

**The consent floors hold whatever a theme changed** (ADR 0097 §6), as it is written and in
every workspace tint, and two looks are compared by the colours they draw, never by the tokens'
names:

- an answer that grants (`grants`, the variant of an Allow or an Accept) keeps its accent edge:
  its `border-width` 0 is refused, a theme that turns every border off still draws it, and its
  edge contrasts 3:1 with its fill;
- Allow is distinguishable from Keep blocked (`secondary`) by its edge, and as plain as Keep
  blocked: in every state it is drawn on Keep blocked's fill;
- no part draws Allow or danger nearer the primary than the family's defaults draw it in the
  same theme, measured on what a person reads as the look as it shows on each surface;
- danger's words stay apart from the accent and from the primary's words.

**A theme's tokens alone keep Allow apart from the primary** (#2948): the button family's
defaults, drawn from the theme's tokens, are measured the same way, at rest and in each state, as
written and in each of the theme's own tints. A theme whose tokens bring the two together is
drawn with purlis's own fill for the primary, and Allow's own fill only when that is not enough,
under the consent floors' Notice.

A consent variant that breaks one draws its family default there, and where that is not enough,
so does the variant it is held apart from. Allow's lock is the window's, and no property reaches
it.

**Sizes.** A size (`padding`, `gap`, `font-size`, `font-weight`, `height`, `border-width`) is
the family's default size, md: it never reaches a button drawn at sm or lg, or one in a row that
wraps, whose size the family keeps, and an sm or lg button's corners are clamped to its own
height. A button names one variant, so a variant's rule reaches only a button of that variant.
A text input's size is its md size too: it never reaches an sm or lg box.

**The window's own families** (manifest version 4, #2913): `panel` (a region: `side`, `content`,
`bottom`), `panel-section` (its `head` and its `title`), `empty-state` (its `icon`, `headline`,
`body` and `action`) and `status-item` (a status bar item or a title-bar pill: `bar`, `pill`,
`waiting`, `accent`; its `icon`, `label` and `count`). A panel, a section and an empty state are
boxes as tall as what they hold: no target of their own, so only their bounds hold, and each
of their corners is clamped to half their own box (`min(r, 50%)`; `full` is `50%`). A status item is a control, 24px tall in either density, and keeps
its words in that height. `panel.root.corner-radius` sets one panel's corners over the theme's
`panel.corner-radius` token, which sets every panel's.
**The tooltip** (`tooltip`, manifest version 5, #2908) has a `root`, its surface, and a `detail`, an icon button's
second line, and since the next release's manifest version (#2921) a `shortcut`, the key that does what its target
does, after its words. A theme sets the surface's fill, words, edge colour and width, corners and shadow;
its size is its words', so none of its sizes is a theme's. Purlis draws it with corners of 8 and
no edge: `"tooltip.root.corner-radius": 8`, `"tooltip.root.border-width": 0`.

**The activity family** (manifest version 10, #2917; its words reworded in the next release's manifest version, #3144): `activity`, a
view's icon on a side's bar: `bar` (an icon in a side's column, the left bar beside the project
rail and the right one at the window's edge) and `tab` (tabs across a side panel's top, which no
bar of the window draws since the left bar became a column); its `item`
(the root: colours, edge and corners, never a size), its `icon`, its `count` and its
`indicator`, the mark along a chosen item's edge. Purlis draws the family default; Purlis
Classic's own `parts` give its square items and accent edge (`activity.indicator.selected.fill`).

**The panel separator** (the next release's manifest version, #3143): `panel-separator`, the handle between two
panels that resizes them (a side's edge, the line under the work, the line between the bottom
panel's halves, a split between panes). Its `root` is the handle and its `grip` a short mark at
its middle, drawn over what is either side of it, so a wider look never changes the handle's box.
Each takes only `fill`, in the states `hover`, `pressed` (dragged) and `focused` (the keyboard
on it), keyed on the panel library's own marks and `:focus-visible` (the manifest's
`attributes`), not React Aria's. It is drawn in marks (the manifest's `marks`): what it draws in
a state is held at 3:1 on its grounds as a non-text mark, a focused handle draws a mark of its
own, and a part that breaks either draws its family default there. At rest a handle parts its
panels as the regions part: by the window's gutter where a theme floats its panels (two panes in
a split sit `window.gutter` apart, in the window's colour), and by the hairline
(`panel.border-width`) where they sit flush. Purlis draws the family default: no grip at rest, a
faint grip under the pointer, a muted one while dragged and one in `focus.ring` with the keyboard
on it, in place of a ring the handle's length. Purlis Classic's own `parts` draw its accent line in all three and no grip
(`panel-separator.root.hover.fill: accent.base`, `panel-separator.grip.hover.fill: transparent`,
and the same for `pressed` and `focused`).

The strip tabs, a tab's `×` closer, the gear on a strip and the list and tree rows are not
families with parts today: they are drawn as the note under [Geometry](#geometry) says.

### Session tabs

The session tabs are the `tabs` family (#2914): each tab (`tab`, in the variants `chat`, `view`,
`file` and `background`), its `label`, its `×` (`closer`), a `count` on it and an `indicator`;
and, drawn around every tab and taking no variant or state, the `strip` they sit in, the
`layout` that holds the strip and the panel, and that panel (`body`). The editor's toolbar over a
file is the `editor` family's `toolbar`.

**Purlis docks the tabs on the content panel**, and that is the family default: the strip is as
wide as the panel with the first tab on its edge, the chosen tab takes the panel's fill and
flares into it with 8px inverse corners, an unchosen tab under the pointer draws a pill inset 3px
from the top and 5px from the bottom so it never touches the panel, and a tab is as wide as its
name, at most 240px, shrinking with an ellipsis before the strip gives any tab up to its
show-more menu. Purlis Classic keeps its strip through parts:

```json
"parts": {
  "tabs.layout.attach": "strip",
  "tabs.layout.sizing": "equal",
  "tabs.layout.dividers": "on",
  "tabs.tab.corner-radius": 0,
  "tabs.tab.hover.inset": 0,
  "tabs.tab.selected.fill": "layer.selected",
  "tabs.tab.selected.flare": 0
}
```

The layout's properties take words: `attach` is `panel` or `strip`, `sizing` `content` or
`equal`, `dividers` `on` or `off`; its `gap` is px at the default text size, up to 32.
`tabs.tab.selected.flare` (0 to 16px, or a radius token) is set only on the selected state, and
`tabs.tab.hover.inset` (one value or three: top, sides, bottom, each 0 to 8px) only on hover:
both are drawn out of the tab's flow (a flare at most as wide as the gap), so they are the two sizes a state may set. Every other size
inside a state is refused, `tabs.tab.selected.font-weight` among them: choosing a tab never
changes its width. `tabs.tab.max-width` is 48 to 480. `tabs.indicator.height` is a mark's, 0 to
6px, drawn on the chosen tab's `top` or `bottom` edge (`tabs.indicator.position`). A chosen tab
takes the fill of what it joins: a theme that gives the panel (`tabs.body.fill`) or the editor's
toolbar (`editor.toolbar.fill`) a fill draws the chosen tab, or the chosen file tab, in it too,
unless it names the tab's own. The strip's line is under it (`tabs.strip.border-width`).

## The theme check

**The grey tiers** (#2902). A theme draws words in four tiers, `text.primary`,
`text.secondary`, `text.muted` and `text.quiet`, and what is not words (a tab's ×, a separator,
a pin, a chevron, and disabled words) in `text.faint`. Quiet is informational text: line
numbers, placeholders, counts, paths and group labels; none of it is exempt, and none of it
takes faint. The check measures every text tier at 4.5:1 and faint at 3:1 on every ground a
part draws them on: the window, a field, the status bar, a panel, a card, the row under the
pointer, the editor's code, the selected row, the tab in front and a control, at rest and
under the pointer. **No theme draws a tier under its floor** (ruling #2803·3, ADR 0098's
amendment of 2026-10-21): a tier that falls under it on one of those grounds, as written or in a
workspace tint, is drawn as the nearest colour that clears it, the theme's own colour walked
along its lightness with its hue kept, and the check says so as a `tier-falls-back` warning
beside the pair's own. A lifted tier can read as loud as the tier above it: the floor comes
before the order. Where no lightness clears every ground (a dark window with a light code
ground), the floor is held on the window's own surfaces, a tier that already clears them keeps
its colour, and the check names the ground on the other side. **A tier the layer below draws in the grey of the tier above it follows
that tier** (ADR 0097's amendment of 2026-10-21): over Purlis Classic, whose quiet and faint are
its muted, a theme that names `text.muted` and not `text.quiet` draws quiet in its muted, and one
that names no `text.faint` draws faint in its quiet. So a theme written before the tiers draws
them as legibly as its muted words. Over a parent with tiers of its own (Purlis), a theme that
changes only `text.muted` keeps the parent's quiet and faint; it names them to change them. A
tint recipe names each token it tints, the tiers included.

`purlis theme check <theme>` checks one theme, named by its id, by the path of a theme file, or
by the path of a theme folder (a folder holding `theme.json`; the folders beside it are the
themes it may `extend`). A level's theme folder is named by its level and folder
(`project:team`) and checked exactly as the window reads it: that level's folder, never a
farther level's of the same name, extending what a choice made at that level reaches. A file
path that is a link is followed, since it is the path you typed; a theme folder, or its
`theme.json`, that is a link is refused:

```
Dim — /home/me/dim.json, dark
left out (1):
  text.primry is not a purlis token, so it is left out
warnings (2):
  text.muted on surface.base: 2.38, under 4.5; in every workspace tint too, down to 2.38
  text.muted on surface.hover, in the red, blue tints: down to 2.07, under 4.5
```

Then `consent floors:`, what the window draws of the theme. Every combination of appearance
and option values is checked against the consent floors as the window draws it, with the option
values layered first, and the line says in how many of them the window holds one (`none held
in 4 combinations`, `held in 1 of 4 combinations:`). Under it, each combination that breaks one names the floors it breaks and
the tokens the window draws as purlis's own for them (`dark, ember on: danger is
distinguishable from primary — purlis draws its own danger.base`). Last, `hard floors:` says each hard floor
a combination breaks as the window draws it, the consent floors held first, with the pair and
each layer under it as a warning names them, its ratio and the tint it was found in
(`hard floors: broken in 1 of 4 combinations:`, then
`dark, ember on: primary text is readable on every surface: text.primary on surface.sunken is
1.12, under 3`), or `none broken in 4 combinations`. A theme some level chose says so under its name: `chosen by: project, committed
(accent=teal); machine, yours`, each level with its option values.

- **Refused**: a theme that breaks its grammar anywhere is not drawn at all, and each value
  that refused it is listed: a colour that is not hex or a palette name, a duration or an
  easing out of bounds, a key `theme.json` does not have, a missing name, text that is not
  JSON. When it is your own `theme.json`, the window draws Purlis in its place and the Inbox
  names the problem, with **Use built-in** to move the file aside.
- **Left out**: a token or motion name purlis does not have, or a key a format 1 file does not
  have, is said and left out; the rest of the theme is drawn.
- **Warnings** are contrast findings in every combination of appearance and option values: each
  pair of tokens that is drawn as text on a surface, with its ratio and the floor it is under,
  as the theme is and tinted by every workspace colour, one line per pair. With more than one
  combination each line starts with the combination: the appearance's id, then each option's
  value in the theme's order of its options (`dark, borders on: …`). A theme with options says
  them on an `options:` line under its name, with how many combinations were checked. A warning
  is the author's call; the theme is drawn.
- **Consent floors** (an ask contrasts with its surface, Allow is distinguishable from Keep
  blocked, the danger colours contrast with what they are drawn on, danger is distinguishable
  from primary, the state marks meet the mark floor, Allow is distinguishable from the primary)
  hold in every combination. A combination
  that breaks one, an option value's included, is drawn with purlis's own values for those
  tokens and the rest as written, and the window says so in a Notice.
- **Hard floors** are the few rules that keep the window readable whatever a theme says, each
  at 3:1 as the window draws the theme, as it is and in every workspace tint: **primary text is
  readable on every surface** (`text.primary` on each ground it is drawn on), **the focus ring is
  seen on its surfaces** (`focus.ring`), and **the terminal's words are readable**
  (`terminal.foreground` on the terminal and its find matches). A theme draft that breaks one is
  not drawn (below); for a kept theme, a broken hard floor is a finding, and the theme is
  drawn. **A hard floor covers colour only**: a face, a size or a spacing can still make words
  hard to read. For a theme draft, the revert key is the way out (below).

The exit is a linter's: **0** when the theme is drawn (notes and warnings or not), **1** when it
is refused, **2** when there is no such theme.

`--json` is the same check for a chat:

```json
{
  "schema": 1,
  "theme_schema": "https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json",
  "theme": {"id": "mine", "name": "Dim",
            "appearances": [{"id": "dark", "appearance": "dark", "high_contrast": false}],
            "options": [], "combinations": 1, "level": "machine",
            "from": {"kind": "mine", "path": "/home/me/.config/purlis/theme.json"}},
  "refused": null,
  "errors": [],
  "notes": [{"said": "text.primry is not a purlis token, so it is left out",
             "token": "text.primry"}],
  "warnings": [{"appearance": "dark", "options": {}, "kind": "contrast", "tint": null,
                "pair": {"front": "text.muted", "back": "surface.base", "floor": 4.5},
                "ratio": 2.38}],
  "held": [],
  "hard_floors": [],
  "clean": false
}
```

`refused` is `null`, or why the theme is not drawn, and `errors` then lists each value that
refused it (`said`, and `token` when it was about one); `name` and `appearances` are `null`.
An error or a note said of a theme folder of a level, or of that level's `themes/`, has `at`:
the folder as its `level` (`machine`, `project`, `workspace` with its `name`, or `draft`) and
its `path` inside that level's folder, `themes/dusk` for a project's folder `dusk`. Its `said`
names a file the same way, "the project's themes/dusk/theme.json", never where the level is on
disk.
`notes` is what was left out of a theme that is drawn. A warning's `appearance` is the id of the
appearance it was found in, and its `options` the value of each option it was drawn with (`{}`
for a theme with none), in the theme's order of its options. Its `kind` is `contrast` (a `pair`,
its `ratio`, and `under` when the back is a wash laid over another token, and `beneath` when
that token is a wash laid over another in turn), `ansi` (a terminal colour too close to the
terminal's background: `token`, `ratio`, `floor`) or `selected-not-apart` (the selected row
drawn exactly as `other`), `selection-text-alone` (a colour for selected text on a selection
left to the platform), `line-does-not-fit` (a line of words taller than the box it is drawn in:
its `density`, the `fit` it breaks, a `size`, a `line_height` and a `height` token, and the
`line` and the `room` it has, in px at the default text size), or `style-without-a-face` (a
style the theme draws in a font it brings with no face for it, so the browser makes it from
another face: the font's `slot`, `ui`, `mono` or `terminal`, the `weight` and the `style`,
`normal` or `italic`). A field (`.ui-input`, `size.control` tall) holds the window's words
(`font-size.base`) and a strip a tab's title (`font-size.md`), each on the body line
(`line-height.body`), in both densities. A button's label sits on a line of 1, so it fits
wherever a field's text does. `tint` is the workspace colour it was found in, or `null` for the
theme as it is, for a line and for a style; a line and a style are found once for each
combination, never for a tint. `held` has one entry for each combination in which the window
holds a consent floor: its `appearance` and `options` as a warning's, the `floors` (`ask`,
`allow-apart`, `danger`, `danger-apart`, `state-marks`) and the `tokens` drawn as purlis's own.
`hard_floors` has one entry for each hard floor a combination breaks: its `appearance` and
`options` as a warning's, the `floor` (`primary-text`, `focus-ring`, `terminal-text`), the
`front` and `back` tokens (`under` for a wash, and `beneath` when that is a wash laid over
another in turn, as a warning's pair), the `ratio` and the `tint`, `null` for the theme as it
is; `[]` means none is broken. `clean` is true with no error, no note, no warning and no floor
held; it covers those only, never hard floors, which `hard_floors` reports on its own.
`schema` is the version of this shape, in both answers: a reader checks it first. A theme
folder's `from` is `{"kind": "folder", "path"}`, a theme file's `{"kind": "file", "path"}`.
`chosen` lists each level that chose the theme checked, nearest first, as `list --json`'s
`choices` does (`[]` for none, and always for a chat's `draft:<folder>`, which no level
chooses).

## Theme drafts

A chat's writes to a theme go to a **theme draft**: a copy of a theme folder in purlis's
git-ignored local folder in the plane, one folder for each chat, named by the chat's id:

```
.purlis/theme-drafts/<chat id>/<theme folder>/theme.json
```

Nothing there is committed, and **no chat writes it**: the app that started a chat writes that
chat's drafts, in its folder, when the chat asks. In a chat purlis started:

- `purlis theme draft <folder>` starts the draft (a copy of the workspace's or the project's
  theme folder of that name, or a new theme that extends Purlis, the default) and makes a working copy
  for the chat to edit, in its temporary folder under the chat's id (`--from <dir>` names
  another), made again from the draft whenever the draft is made anew;
- `purlis theme draft <folder> --save` hands the working copy's `theme.json` and icon sets to the
  app, which writes them into the draft and says what the window draws of it: drawn, or not
  drawn and which hard floor it breaks (exit 1). A save carries at most 64 KiB as JSON writes
  it (Purlis Classic's whole `theme.json` is about a fifth of that), and a larger one is refused
  before it is sent;
- `purlis theme check draft:<folder>` checks the draft exactly as the window draws it, and exits
  1 when it breaks a hard floor;
- the `theme_preview` tool draws the theme specimen of `draft:<folder>`, the chat's own draft, and
  can draw it side by side with the theme as kept (`side_by_side: {"theme": "<folder>"}`). A chat
  is drawn only its own drafts, and a draft that breaks a hard floor is not drawn.

A draft's fonts are the ones its folder already has, as approved with the project; a save hands
over theme files only.

- **A draft is drawn only in its chat's workspace** (the plane root for a chat there), in place
  of what the levels choose, while that chat is open. purlis knows where each of its chats works;
  other workspaces and the plane root draw as before. A save repaints, as any theme folder's
  does.
- **A draft is read as a project's theme folder is**: through handles refusing a link, under the
  caps of a theme folder. Its fonts follow the project's approval: a face is drawn only when the
  project's approval holds that file, with that digest, in the folder the draft is kept to. A
  draft refused for such a face is said in a Notice, which offers Review and allow by the rules
  of any refused theme, and only where the project itself brings that file, with those bytes, in
  the folder the draft is kept to: a face only the draft has is no project's to allow.
- **A draft that breaks a hard floor is not drawn**, in any combination of its appearances and
  option values: the last good version of it stays drawn, or the kept theme when there is none,
  and the chat is told which floor (by `--save`'s answer, and by `purlis
  theme check draft:<folder>`). The consent floors hold on a draft as on any theme.
- **A strip on the workspace** says `Theme draft — <folder>, edited by <chat>`, with **Keep**,
  **Revert** and **Compare**. **Keep** copies the draft into the real folder it is a draft of (the
  workspace's theme folder of that name, else the project's; a new theme goes in the chat's own
  level) and deletes the draft, then asks once whether the theme becomes the workspace's (or the
  project's) committed default or stays your own choice there. The strip says where Keep puts it
  before you press it (`Keep puts it in the project's themes, for every workspace`), and Keep asks
  before it replaces a theme folder that is there, saying when that folder is not what the draft
  started from: a draft's start records the folder it copied, and a change to it since (your own
  edit, a pull, another chat's write) is said before Keep replaces it. Keep does what you were
  shown, or nothing: when where it puts the draft, or the folder there, changed since, it is
  refused. Only a draft the window draws is kept, with
  the files that were checked, and only the font files the theme names and the project
  approved. **Revert** deletes the draft. **Compare** draws the theme as kept while it is held.
  While a draft is drawn, the Notice of a kept theme that is refused waits, and the strip says
  so; Revert brings it back.
- **The revert key**, ⌘⇧⌫ on macOS and Ctrl+Shift+Backspace elsewhere, deletes every draft
  drawn where the window is, at once. It is the way out of a draft whose face, size or spacing
  makes the window hard to read, which no hard floor measures: it works whatever the window
  looks like.
- **Closing a chat** that owns a draft asks Keep or Revert, saying where Keep puts each and what
  it replaces; neither close is offered until it is answered. A close that ends several chats
  asks about the drafts of each, and Keep then asks who draws each theme it kept. Where purlis
  cannot tell whether a chat has a draft, the close is refused, with why. A chat is closed in
  purlis's own window, and a link is told nothing of its drafts: a link's close of a chat that
  has drafts is handed to the window, which asks Keep or Revert there and says a link asked,
  never over a question already on screen.
- **The drafts of a chat that is no longer open are deleted**: once purlis's record of the open
  chats no longer holds it (it was closed, or a launch did not reopen it), nothing names its
  drafts, and they go. Each chat's drafts folder names the clone whose purlis made it, and only
  that clone deletes it: clones that share one state folder (`$PURLIS_HOME`) keep each other's
  drafts. A drafts folder that names no clone is adopted only in the plane's own state folder,
  never under `$PURLIS_HOME`: purlis records this clone as its owner, and deletes it once its
  chat is gone. A `$PURLIS_HOME` another clone sets to this plane's own state folder counts as
  this clone's own.
- **Moving a theme up a level**: in Settings › Theme, **Use in the whole project** moves a
  workspace's theme folder to the project's `themes/`, and **Copy to my machine** copies a
  project's or a workspace's to your own, carrying only the font files you approved with the
  project. Neither replaces a folder of that name that is there. A theme in the project's
  `themes/` is committed for everyone on the project's next save, even when it came from a
  workspace that is local.

## Making a theme with a chat

purlis's **theme** skill guides a chat through making a theme: from a brand colour, an image or a
screenshot, a Figma frame, an imported theme or another theme. It also edits, checks and shares
one. The chat works through a theme draft, so the theme is drawn live where the chat works, and
it loops: save the draft, look at the theme specimen (`theme_preview`), `purlis theme check
draft:<folder>`, fix. It finishes with a clean check, or names each warning it leaves and why.
The person keeps or reverts the draft on its strip.

A chat started in a workspace makes that workspace's theme; one at the plane root makes the
project's. Start one:

- by asking any chat for a theme, which reaches the skill by its name;
- with **Create a theme** in a workspace's or the plane's **Curate** menu;
- from Settings › Theme at the Project or Workspace level: **New theme**, or **Edit with a chat**
  on the chosen theme, which names that theme by its id from `purlis theme list`.

Each opens a chat with its prompt typed and never sent: read it and press Enter. A read-only
project keeps no theme, so Settings › Theme offers neither button there and says why.

## The schema a theme names

A theme's `$schema` names the published schema, so an editor completes token names and flags a
mistake as you type. Name the one of the purlis the theme is for, at its release:

```
https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json
```

`<version>` is what `purlis --version` prints; `purlis theme list --json` gives the whole
address as `theme_schema`. An address at `main` would offer tokens your purlis does not have
yet, and it leaves those out. The `v2` is the format: within it the schema only ever grows, so
a theme valid against one release's schema is valid against every later release's. purlis
itself never fetches the address; only your editor does.

### Tokens added after themes existed

The Figma pass added eight keys to format 2 (operator ruling 2026-10-25, #3177, #3113):
`border.control` (a plain control's edge: the secondary button's, a field's, a select's),
`accent.wash` (a chosen activity button's ground), `accent.text` (words and icons in the
accent on its surface or wash), `accent.fill`, `accent.fill-hover`, `accent.fill-edge` and
`accent.fill-text` (the primary button's ground, its ground under the pointer, its edge and its
words, held to 4.5:1), and the geometry token `radius.control` (an md control's corner). **A
theme that leaves one out draws as it did before it existed**: while the theme it extends draws
the new token and the token that drew its part before alike, as Purlis Classic does, the new one
follows the value the theme gives the old one (`vocabulary::KIN`):

| new | follows |
|---|---|
| `border.control` | `border.strong` |
| `accent.wash` | `accent.surface` |
| `accent.text` | `accent.base` |
| `accent.fill` | `accent.surface` |
| `accent.fill-hover` | `tab.active` |
| `accent.fill-edge` | `accent.base` |
| `accent.fill-text` | `text.primary` |
| `accent.wash-hover` | `tab.active` |

Later, `accent.wash-hover` came in (#3345): the chosen title-bar chip's ground under the pointer,
a step stronger than `accent.wash`, which is the chosen chip's ground at rest; both hold the
primary words at 4.5:1 over the title bar.

Over Purlis, whose primary is filled and whose controls draw no edge, a theme keeps Purlis's
values unless it names them. `radius.control` is Classic's `radius.md`, 4, where a theme names
neither.

## Importing a theme

`purlis theme import <file>` makes a theme folder from a theme you already have:

- a **VS Code colour theme** (`--kind vscode`): its JSON, comments and trailing commas allowed;
- a **base16** or **base24 scheme** (`base16`, `base24`): YAML or JSON, the classic form
  (`scheme:`, `base00: "1b2024"`) or the tinted one (`system:`, `name:`, `variant:`,
  `palette:`);
- an **iTerm2 colour preset** (`iterm`), an `.itermcolors` file;
- a **Ghostty theme** (`ghostty`): its `key = value` lines;
- a **theme folder's zip** (`zip`), as `purlis theme export` writes one;
- a **W3C design-tokens file** (`design-tokens`), format 2025.10, as `purlis theme export --kind
  design-tokens` or a design tool writes one: see [Design tokens](#design-tokens).

The kind is told from the file's name and its first words; `--kind` says it when they do not.
The new folder goes in `--into <folder>`, a `themes/` folder, or in your own themes' folder,
`themes/` in purlis's config folder, where `purlis theme list` lists it as `machine:<folder>`.
It is named from the theme's name (`Solarized Dark` goes in
`solarized-dark/`): a theme folder's id is lower-case letters, digits and dashes, starting with a
letter, at most 32 long, never a name Windows keeps for a device (see below), and never one
purlis keeps for its own themes (`purlis-classic`, the ids its appearances have had, `mine`,
`system`). A folder of that name that is there is never
replaced without `--force`, and with it, it is swapped for the new one in one step:

```
imported Harbor (base16) into /home/me/.config/purlis/themes/harbor
not taken (2):
  author: theme.json has no place for it
  base06: no token takes it
Harbor — /home/me/.config/purlis/themes/harbor, dark
left out: nothing
warnings: none
the theme is in /home/me/.config/purlis/themes/harbor: `purlis theme check /home/me/.config/purlis/themes/harbor` checks it again, and `purlis theme export /home/me/.config/purlis/themes/harbor` makes a zip of it to share
```

- **Each kind goes through a fixed table** (below): each token takes the first of its source
  keys that the source has. A token no row fills is Purlis Classic's of the same appearance, as
  in any theme that leaves a token out.
- **A source's own quiet and disabled greys take the grey tiers** (#2941), and no tier reads
  louder than the one above it. A VS Code theme's line numbers are `text.quiet` and its
  disabled words `text.faint`; its muted words take the louder of the two greys. A base16 or
  base24 scheme's invisibles (`base03`) are `text.faint`, walked to the mark floor. A tier the
  source has no grey for, or whose grey would read louder than the tier above it, is left out
  and follows that tier (see [The theme check](#the-theme-check)): a VS Code theme's faint
  follows its quiet, which follows its muted when it has no line numbers; a base16 or base24
  scheme's quiet follows its muted, and so does its faint when the walk would take it past
  muted; an iTerm2 preset's and a Ghostty theme's quiet and faint are their muted.
- **Then each kind's recipes tune the tokens the table filled** (below each table), so the
  theme reads where the source's own colours allow it. A base16 scheme makes some roles one
  colour (ANSI black is the background; the selected row is the row under the pointer) and
  names hues, not words on a ground. So the row under the pointer is a colour halfway between
  two of the scheme's, selected text is the source's words, and a muted word, a comment or a
  hue that does not read on the grounds it is drawn on walks toward the source's words, a tenth
  of a step at a time along its own scale, and takes the nearest point that leaves the theme
  check the fewest findings (the consent floors it would hold counted with them). A colour that
  reads already is left as the source wrote it, and a walked one is written as hex. A
  terminal's own colours (its background, its words, its sixteen ANSI colours, its cursor and
  its selection) are what it draws, and are never tuned: what the check finds in them is the
  source's own, and the import's warning about one names the palette entry that holds it
  (``…; it is the palette's `palette-0`: change `palette-0` in theme.json to lift it``). The window draws every imported theme through the consent floors, as it draws
  any theme.
- **Nothing is dropped silently.** `not taken` lists every key of the source that gave no token a
  value, with why: no token takes it, a nearer key gave its token the value, it is not a colour, it names another file (a VS Code
  theme's `include`, which import never reads), or `theme.json` has no place for it.
- **The new theme's check follows**, the same as `purlis theme check` of the folder. A theme
  purlis would refuse is not written at all.
- **A scheme's or a terminal's own colours become the theme's palette**, each named after its
  source key (`base0d`, `ansi-4`, `palette-4`, `background`), so a colour changed once changes
  every token it fills. A VS Code theme's colours are written as hex.
- **The appearance** is the source's own word where it has one: VS Code's `type` (`hc` and
  `hcLight` are tagged high contrast), a tinted scheme's `variant`. Otherwise it is dark when the
  background reads better under white text than black. An iTerm2 preset that keeps colours for
  dark and light mode (`Ansi 0 Color (Dark)`) gets a `dark` and a `light` appearance.

A source is read from the file alone, never from another file it names and never from the
network, and is at most 1 MiB; an iTerm2 preset nests its dictionaries and lists at most 8
deep. The exit: **0** when the folder is written, **1** when it is not
(the file cannot be read as its kind, the folder is there, purlis refuses the theme), **2** when
there is no such file.

`--json` answers `{"schema": 1, "kind", "name", "folder", "written", "not_taken": [{"key",
"why"}], "check"}`: `written` is the new folder's path, or `null` when nothing was written, and
`check` is what `purlis theme check --json` answers for it.

### VS Code

A VS Code theme's `colors`:

| Token | Takes, nearest first |
|---|---|
| `surface.base` | `editor.background` |
| `surface.sunken` | `sideBar.background` |
| `surface.deep` | `titleBar.activeBackground`, `activityBar.background` |
| `surface.raised` | `editorWidget.background` |
| `surface.overlay` | `quickInput.background`, `menu.background`, `editorWidget.background` |
| `surface.hover` | `list.hoverBackground` |
| `control.base` | `input.background`, `dropdown.background` |
| `control.hover` | `toolbar.hoverBackground` |
| `text.primary` | `editor.foreground`, `foreground` |
| `text.secondary` | `descriptionForeground` |
| `text.muted` | `editorLineNumber.foreground`, `disabledForeground` |
| `text.quiet` | `editorLineNumber.foreground` |
| `text.faint` | `disabledForeground` |
| `border.subtle` | `panel.border`, `sideBar.border`, `editorGroup.border` |
| `border.strong` | `input.border`, `contrastBorder` |
| `accent.base` | `button.background`, `focusBorder` |
| `focus.ring` | `focusBorder` |
| `tab.active` | `tab.activeBackground` |
| `list.selected` | `list.activeSelectionBackground` |
| `tree.guide` | `tree.indentGuidesStroke` |
| `danger.base` | `errorForeground`, `editorError.foreground` |
| `diff.deleted` | `diffEditor.removedLineBackground`, `diffEditor.removedTextBackground` |
| `diff.inserted` | `diffEditor.insertedLineBackground`, `diffEditor.insertedTextBackground` |
| `diff.deleted-text` | `diffEditor.removedTextBackground` |
| `diff.inserted-text` | `diffEditor.insertedTextBackground` |
| `icon.red` | `charts.red` |
| `icon.orange` | `charts.orange` |
| `icon.yellow` | `charts.yellow` |
| `icon.green` | `charts.green` |
| `icon.blue` | `charts.blue` |
| `icon.purple` | `charts.purple` |
| `state.running` | `progressBar.background` |
| `state.waiting` | `editorWarning.foreground` |
| `state.failed` | `editorError.foreground`, `errorForeground` |
| `state.success` | `testing.iconPassed` |
| `overlay.shadow` | `widget.shadow` |
| `terminal.background` | `terminal.background`, `panel.background`, `editor.background` |
| `terminal.foreground` | `terminal.foreground`, `editor.foreground`, `foreground` |
| `terminal.cursor` | `terminalCursor.foreground`, `editorCursor.foreground` |
| `terminal.cursor-accent` | `terminalCursor.background` |
| `terminal.selection` | `terminal.selectionBackground`, `editor.selectionBackground` |
| `terminal.find-match` | `terminal.findMatchHighlightBackground` |
| `terminal.find-match-active` | `terminal.findMatchBackground` |
| `terminal.selection-foreground` | `terminal.selectionForeground` |
| `terminal.selection-inactive` | `terminal.inactiveSelectionBackground` |
| `terminal.scrollbar` | `scrollbarSlider.background` |
| `terminal.scrollbar-hover` | `scrollbarSlider.hoverBackground` |
| `terminal.scrollbar-active` | `scrollbarSlider.activeBackground` |
| `terminal.ansi.black` | `terminal.ansiBlack` |
| `terminal.ansi.red` | `terminal.ansiRed` |
| `terminal.ansi.green` | `terminal.ansiGreen` |
| `terminal.ansi.yellow` | `terminal.ansiYellow` |
| `terminal.ansi.blue` | `terminal.ansiBlue` |
| `terminal.ansi.magenta` | `terminal.ansiMagenta` |
| `terminal.ansi.cyan` | `terminal.ansiCyan` |
| `terminal.ansi.white` | `terminal.ansiWhite` |
| `terminal.ansi.bright-black` | `terminal.ansiBrightBlack` |
| `terminal.ansi.bright-red` | `terminal.ansiBrightRed` |
| `terminal.ansi.bright-green` | `terminal.ansiBrightGreen` |
| `terminal.ansi.bright-yellow` | `terminal.ansiBrightYellow` |
| `terminal.ansi.bright-blue` | `terminal.ansiBrightBlue` |
| `terminal.ansi.bright-magenta` | `terminal.ansiBrightMagenta` |
| `terminal.ansi.bright-cyan` | `terminal.ansiBrightCyan` |
| `terminal.ansi.bright-white` | `terminal.ansiBrightWhite` |
| `markdown.link` | `textLink.foreground` |
| `markdown.code` | `textPreformat.foreground` |
| `markdown.code-block` | `textPreformat.foreground` |
| `selection.background` | `editor.selectionBackground` |
| `selection.text` | `editor.selectionForeground` |
| `caret` | `editorCursor.foreground` |
| `placeholder` | `input.placeholderForeground` |
| `scrollbar.thumb` | `scrollbarSlider.background` |
| `chart.1` | `charts.blue` |
| `chart.2` | `charts.green` |
| `chart.3` | `charts.yellow` |
| `chart.4` | `charts.purple` |
| `chart.5` | `charts.red` |
| `chart.6` | `charts.orange` |
| `chart.7` | `terminal.ansiCyan` |
| `chart.8` | `terminal.ansiMagenta` |
| `window.background` | `titleBar.activeBackground`, `editor.background` |

Tuned after the table (#2589): a VS Code theme names a colour for nearly every role, and those are taken as they are. Selected text it leaves to the selection is the editor's words. Its muted words take the louder of its line numbers' grey and its disabled grey, and its faint follows quiet when the disabled grey reads louder than the line numbers.

| Token | Tuned |
|---|---|
| `selection.text` | when the theme's `selection.background` is filled and the table left it: `editor.foreground`, `foreground` |
| `text.muted` | the one of `editorLineNumber.foreground`, `disabledForeground` that reads most on `surface.base` |
| `text.faint` | left out when it reads louder than `text.quiet` on `surface.base` |

Its `tokenColors` fill the syntax tokens. A rule's scope fills a token when it is one of the
token's scopes or under one (`keyword.control` is `keyword`'s), the nearest scope winning and,
between rules of one scope, the last; a scope with a space in it (`meta.embedded string`) is
about a context and fills none. A rule's `fontStyle` sets the token's italic and bold.

| Token | Scopes, nearest first |
|---|---|
| `syntax.keyword` | `keyword`, `storage` |
| `syntax.string` | `string` |
| `syntax.number` | `constant.numeric` |
| `syntax.constant` | `constant.language`, `constant`, `support.constant` |
| `syntax.comment` | `comment` |
| `syntax.function` | `entity.name.function`, `support.function` |
| `syntax.type` | `entity.name.type`, `entity.name.class`, `support.type`, `support.class` |
| `syntax.property` | `variable.other.property`, `support.type.property-name`, `meta.object-literal.key` |
| `syntax.attribute` | `entity.other.attribute-name` |
| `syntax.tag` | `entity.name.tag` |
| `syntax.heading` | `markup.heading`, `entity.name.section` |
| `syntax.invalid` | `invalid` |
| `syntax.variable` | `variable` |
| `syntax.operator` | `keyword.operator` |
| `syntax.punctuation` | `punctuation` |
| `markdown.heading` | `markup.heading` |

### base16

| Token | Takes, nearest first |
|---|---|
| `surface.base` | `base00` |
| `surface.sunken` | `base01` |
| `surface.raised` | `base01` |
| `surface.overlay` | `base01` |
| `surface.hover` | `base02` |
| `control.base` | `base01` |
| `control.hover` | `base02` |
| `text.primary` | `base05` |
| `text.secondary` | `base04` |
| `text.muted` | `base03` |
| `text.faint` | `base03` |
| `border.subtle` | `base02` |
| `border.strong` | `base03` |
| `accent.base` | `base0d` |
| `focus.ring` | `base0d` |
| `list.selected` | `base02` |
| `needs-you.base` | `base08` |
| `needs-you.text` | `base00` |
| `danger.base` | `base08` |
| `icon.red` | `base08` |
| `icon.orange` | `base09` |
| `icon.yellow` | `base0a` |
| `icon.green` | `base0b` |
| `icon.teal` | `base0c` |
| `icon.blue` | `base0d` |
| `icon.purple` | `base0e` |
| `icon.pink` | `base0f` |
| `state.running` | `base0d` |
| `state.waiting` | `base0a` |
| `state.failed` | `base08` |
| `state.success` | `base0b` |
| `state.unreadable` | `base09` |
| `terminal.background` | `base00` |
| `terminal.foreground` | `base05` |
| `terminal.cursor` | `base05` |
| `terminal.cursor-accent` | `base00` |
| `terminal.selection` | `base02` |
| `terminal.ansi.black` | `base00` |
| `terminal.ansi.red` | `base08` |
| `terminal.ansi.green` | `base0b` |
| `terminal.ansi.yellow` | `base0a` |
| `terminal.ansi.blue` | `base0d` |
| `terminal.ansi.magenta` | `base0e` |
| `terminal.ansi.cyan` | `base0c` |
| `terminal.ansi.white` | `base05` |
| `terminal.ansi.bright-black` | `base03` |
| `terminal.ansi.bright-red` | `base08` |
| `terminal.ansi.bright-green` | `base0b` |
| `terminal.ansi.bright-yellow` | `base0a` |
| `terminal.ansi.bright-blue` | `base0d` |
| `terminal.ansi.bright-magenta` | `base0e` |
| `terminal.ansi.bright-cyan` | `base0c` |
| `terminal.ansi.bright-white` | `base07` |
| `syntax.keyword` | `base0e` |
| `syntax.string` | `base0b` |
| `syntax.number` | `base09` |
| `syntax.constant` | `base09` |
| `syntax.comment` | `base03` |
| `syntax.function` | `base0d` |
| `syntax.type` | `base0a` |
| `syntax.property` | `base0c` |
| `syntax.attribute` | `base09` |
| `syntax.tag` | `base08` |
| `syntax.heading` | `base0d` |
| `syntax.invalid` | `base08` |
| `syntax.variable` | `base08` |
| `syntax.operator` | `base05` |
| `syntax.punctuation` | `base05` |
| `markdown.link` | `base0d` |
| `markdown.code` | `base0b` |
| `markdown.code-block` | `base05` |
| `markdown.quote` | `base0c` |
| `markdown.heading` | `base0d` |
| `selection.background` | `base02` |
| `caret` | `base05` |
| `placeholder` | `base04` |
| `scrollbar.thumb` | `base03` |
| `chart.1` | `base0d` |
| `chart.2` | `base0b` |
| `chart.3` | `base0a` |
| `chart.4` | `base0e` |
| `chart.5` | `base08` |
| `chart.6` | `base0c` |
| `chart.7` | `base09` |
| `chart.8` | `base0f` |
| `window.background` | `base00` |

Tuned after the table (#2589), in this order. base16 makes some roles one colour, and names hues on a scale rather than words on a ground, so these take a nearer step on the scheme's own scale, or a colour between two of its own:

| Token | Tuned |
|---|---|
| `surface.hover` | halfway between `base01` and `base02` |
| `selection.text` | when the theme's `selection.background` is filled and the table left it: `base05` |
| `terminal.ansi.black` | toward `base01`, then `base02`, then `base03` |
| `text.muted` | toward `base04`, then `base05` |
| `text.faint` | toward `base04`, then `base05` |
| `text.faint` | left out when it reads louder than `text.muted` on `surface.base` |
| `syntax.comment` | toward `base04`, then `base05` |
| `terminal.ansi.bright-black` | toward `base04`, then `base05` |
| `text.secondary` | toward `base05` |
| `placeholder` | toward `base05` |
| `danger.base` | toward `base05` |
| `accent.base` | toward `base05` |
| `focus.ring` | toward `base05` |
| `icon.red` | toward `base05` |
| `icon.orange` | toward `base05` |
| `icon.yellow` | toward `base05` |
| `icon.green` | toward `base05` |
| `icon.teal` | toward `base05` |
| `icon.blue` | toward `base05` |
| `icon.purple` | toward `base05` |
| `icon.pink` | toward `base05` |
| `state.running` | toward `base05` |
| `state.waiting` | toward `base05` |
| `state.failed` | toward `base05` |
| `state.success` | toward `base05` |
| `state.unreadable` | toward `base05` |
| `needs-you.base` | toward `base05` |
| `syntax.keyword` | toward `base05` |
| `syntax.string` | toward `base05` |
| `syntax.number` | toward `base05` |
| `syntax.constant` | toward `base05` |
| `syntax.function` | toward `base05` |
| `syntax.type` | toward `base05` |
| `syntax.property` | toward `base05` |
| `syntax.attribute` | toward `base05` |
| `syntax.tag` | toward `base05` |
| `syntax.heading` | toward `base05` |
| `syntax.invalid` | toward `base05` |
| `syntax.variable` | toward `base05` |
| `markdown.link` | toward `base05` |
| `markdown.code` | toward `base05` |
| `markdown.quote` | toward `base05` |
| `markdown.heading` | toward `base05` |
| `chart.1` | toward `base05` |
| `chart.2` | toward `base05` |
| `chart.3` | toward `base05` |
| `chart.4` | toward `base05` |
| `chart.5` | toward `base05` |
| `chart.6` | toward `base05` |
| `chart.7` | toward `base05` |
| `chart.8` | toward `base05` |
| `terminal.ansi.red` | toward `base05` |
| `terminal.ansi.green` | toward `base05` |
| `terminal.ansi.yellow` | toward `base05` |
| `terminal.ansi.blue` | toward `base05` |
| `terminal.ansi.magenta` | toward `base05` |
| `terminal.ansi.cyan` | toward `base05` |
| `terminal.ansi.bright-red` | toward `base05` |
| `terminal.ansi.bright-green` | toward `base05` |
| `terminal.ansi.bright-yellow` | toward `base05` |
| `terminal.ansi.bright-blue` | toward `base05` |
| `terminal.ansi.bright-magenta` | toward `base05` |
| `terminal.ansi.bright-cyan` | toward `base05` |

### base24

| Token | Takes, nearest first |
|---|---|
| `surface.base` | `base00` |
| `surface.sunken` | `base10`, `base01` |
| `surface.deep` | `base11`, `base10` |
| `surface.raised` | `base01` |
| `surface.overlay` | `base01` |
| `surface.hover` | `base02` |
| `control.base` | `base01` |
| `control.hover` | `base02` |
| `text.primary` | `base05` |
| `text.secondary` | `base04` |
| `text.muted` | `base03` |
| `text.faint` | `base03` |
| `border.subtle` | `base02` |
| `border.strong` | `base03` |
| `accent.base` | `base0d` |
| `focus.ring` | `base0d` |
| `list.selected` | `base02` |
| `needs-you.base` | `base08` |
| `needs-you.text` | `base00` |
| `danger.base` | `base08` |
| `icon.red` | `base08` |
| `icon.orange` | `base09` |
| `icon.yellow` | `base0a` |
| `icon.green` | `base0b` |
| `icon.teal` | `base0c` |
| `icon.blue` | `base0d` |
| `icon.purple` | `base0e` |
| `icon.pink` | `base0f` |
| `state.running` | `base0d` |
| `state.waiting` | `base0a` |
| `state.failed` | `base08` |
| `state.success` | `base0b` |
| `state.unreadable` | `base09` |
| `terminal.background` | `base00` |
| `terminal.foreground` | `base05` |
| `terminal.cursor` | `base05` |
| `terminal.cursor-accent` | `base00` |
| `terminal.selection` | `base02` |
| `terminal.ansi.black` | `base00` |
| `terminal.ansi.red` | `base08` |
| `terminal.ansi.green` | `base0b` |
| `terminal.ansi.yellow` | `base0a` |
| `terminal.ansi.blue` | `base0d` |
| `terminal.ansi.magenta` | `base0e` |
| `terminal.ansi.cyan` | `base0c` |
| `terminal.ansi.white` | `base05` |
| `terminal.ansi.bright-black` | `base03` |
| `terminal.ansi.bright-red` | `base12`, `base08` |
| `terminal.ansi.bright-green` | `base14`, `base0b` |
| `terminal.ansi.bright-yellow` | `base13`, `base0a` |
| `terminal.ansi.bright-blue` | `base16`, `base0d` |
| `terminal.ansi.bright-magenta` | `base17`, `base0e` |
| `terminal.ansi.bright-cyan` | `base15`, `base0c` |
| `terminal.ansi.bright-white` | `base07` |
| `syntax.keyword` | `base0e` |
| `syntax.string` | `base0b` |
| `syntax.number` | `base09` |
| `syntax.constant` | `base09` |
| `syntax.comment` | `base03` |
| `syntax.function` | `base0d` |
| `syntax.type` | `base0a` |
| `syntax.property` | `base0c` |
| `syntax.attribute` | `base09` |
| `syntax.tag` | `base08` |
| `syntax.heading` | `base0d` |
| `syntax.invalid` | `base08` |
| `syntax.variable` | `base08` |
| `syntax.operator` | `base05` |
| `syntax.punctuation` | `base05` |
| `markdown.link` | `base0d` |
| `markdown.code` | `base0b` |
| `markdown.code-block` | `base05` |
| `markdown.quote` | `base0c` |
| `markdown.heading` | `base0d` |
| `selection.background` | `base02` |
| `caret` | `base05` |
| `placeholder` | `base04` |
| `scrollbar.thumb` | `base03` |
| `chart.1` | `base0d` |
| `chart.2` | `base0b` |
| `chart.3` | `base0a` |
| `chart.4` | `base0e` |
| `chart.5` | `base08` |
| `chart.6` | `base0c` |
| `chart.7` | `base09` |
| `chart.8` | `base0f` |
| `window.background` | `base00` |

Tuned as a base16 scheme is, after the table (#2589):

| Token | Tuned |
|---|---|
| `surface.hover` | halfway between `base01` and `base02` |
| `selection.text` | when the theme's `selection.background` is filled and the table left it: `base05` |
| `terminal.ansi.black` | toward `base01`, then `base02`, then `base03` |
| `text.muted` | toward `base04`, then `base05` |
| `text.faint` | toward `base04`, then `base05` |
| `text.faint` | left out when it reads louder than `text.muted` on `surface.base` |
| `syntax.comment` | toward `base04`, then `base05` |
| `terminal.ansi.bright-black` | toward `base04`, then `base05` |
| `text.secondary` | toward `base05` |
| `placeholder` | toward `base05` |
| `danger.base` | toward `base05` |
| `accent.base` | toward `base05` |
| `focus.ring` | toward `base05` |
| `icon.red` | toward `base05` |
| `icon.orange` | toward `base05` |
| `icon.yellow` | toward `base05` |
| `icon.green` | toward `base05` |
| `icon.teal` | toward `base05` |
| `icon.blue` | toward `base05` |
| `icon.purple` | toward `base05` |
| `icon.pink` | toward `base05` |
| `state.running` | toward `base05` |
| `state.waiting` | toward `base05` |
| `state.failed` | toward `base05` |
| `state.success` | toward `base05` |
| `state.unreadable` | toward `base05` |
| `needs-you.base` | toward `base05` |
| `syntax.keyword` | toward `base05` |
| `syntax.string` | toward `base05` |
| `syntax.number` | toward `base05` |
| `syntax.constant` | toward `base05` |
| `syntax.function` | toward `base05` |
| `syntax.type` | toward `base05` |
| `syntax.property` | toward `base05` |
| `syntax.attribute` | toward `base05` |
| `syntax.tag` | toward `base05` |
| `syntax.heading` | toward `base05` |
| `syntax.invalid` | toward `base05` |
| `syntax.variable` | toward `base05` |
| `markdown.link` | toward `base05` |
| `markdown.code` | toward `base05` |
| `markdown.quote` | toward `base05` |
| `markdown.heading` | toward `base05` |
| `chart.1` | toward `base05` |
| `chart.2` | toward `base05` |
| `chart.3` | toward `base05` |
| `chart.4` | toward `base05` |
| `chart.5` | toward `base05` |
| `chart.6` | toward `base05` |
| `chart.7` | toward `base05` |
| `chart.8` | toward `base05` |
| `terminal.ansi.red` | toward `base05` |
| `terminal.ansi.green` | toward `base05` |
| `terminal.ansi.yellow` | toward `base05` |
| `terminal.ansi.blue` | toward `base05` |
| `terminal.ansi.magenta` | toward `base05` |
| `terminal.ansi.cyan` | toward `base05` |
| `terminal.ansi.bright-red` | toward `base05` |
| `terminal.ansi.bright-green` | toward `base05` |
| `terminal.ansi.bright-yellow` | toward `base05` |
| `terminal.ansi.bright-blue` | toward `base05` |
| `terminal.ansi.bright-magenta` | toward `base05` |
| `terminal.ansi.bright-cyan` | toward `base05` |

### iTerm2

| Token | Takes, nearest first |
|---|---|
| `surface.base` | `Background Color` |
| `text.primary` | `Foreground Color` |
| `text.muted` | `Ansi 8 Color` |
| `accent.base` | `Ansi 4 Color` |
| `focus.ring` | `Ansi 4 Color` |
| `danger.base` | `Ansi 1 Color` |
| `icon.red` | `Ansi 1 Color` |
| `icon.yellow` | `Ansi 3 Color` |
| `icon.green` | `Ansi 2 Color` |
| `icon.teal` | `Ansi 6 Color` |
| `icon.blue` | `Ansi 4 Color` |
| `icon.purple` | `Ansi 5 Color` |
| `state.running` | `Ansi 4 Color` |
| `state.waiting` | `Ansi 3 Color` |
| `state.failed` | `Ansi 1 Color` |
| `state.success` | `Ansi 2 Color` |
| `terminal.background` | `Background Color` |
| `terminal.foreground` | `Foreground Color` |
| `terminal.cursor` | `Cursor Color` |
| `terminal.cursor-accent` | `Cursor Text Color` |
| `terminal.selection` | `Selection Color` |
| `terminal.selection-foreground` | `Selected Text Color` |
| `terminal.ansi.black` | `Ansi 0 Color` |
| `terminal.ansi.red` | `Ansi 1 Color` |
| `terminal.ansi.green` | `Ansi 2 Color` |
| `terminal.ansi.yellow` | `Ansi 3 Color` |
| `terminal.ansi.blue` | `Ansi 4 Color` |
| `terminal.ansi.magenta` | `Ansi 5 Color` |
| `terminal.ansi.cyan` | `Ansi 6 Color` |
| `terminal.ansi.white` | `Ansi 7 Color` |
| `terminal.ansi.bright-black` | `Ansi 8 Color` |
| `terminal.ansi.bright-red` | `Ansi 9 Color` |
| `terminal.ansi.bright-green` | `Ansi 10 Color` |
| `terminal.ansi.bright-yellow` | `Ansi 11 Color` |
| `terminal.ansi.bright-blue` | `Ansi 12 Color` |
| `terminal.ansi.bright-magenta` | `Ansi 13 Color` |
| `terminal.ansi.bright-cyan` | `Ansi 14 Color` |
| `terminal.ansi.bright-white` | `Ansi 15 Color` |
| `syntax.keyword` | `Ansi 5 Color` |
| `syntax.string` | `Ansi 2 Color` |
| `syntax.number` | `Ansi 11 Color` |
| `syntax.constant` | `Ansi 11 Color` |
| `syntax.comment` | `Ansi 8 Color` |
| `syntax.function` | `Ansi 4 Color` |
| `syntax.type` | `Ansi 3 Color` |
| `syntax.property` | `Ansi 6 Color` |
| `syntax.tag` | `Ansi 1 Color` |
| `syntax.heading` | `Ansi 4 Color` |
| `syntax.invalid` | `Ansi 1 Color` |
| `syntax.variable` | `Ansi 1 Color` |
| `markdown.link` | `Link Color`, `Ansi 4 Color` |
| `markdown.code` | `Ansi 2 Color` |
| `markdown.quote` | `Ansi 6 Color` |
| `markdown.heading` | `Ansi 4 Color` |
| `selection.background` | `Selection Color` |
| `selection.text` | `Selected Text Color` |
| `caret` | `Cursor Color` |
| `chart.1` | `Ansi 4 Color` |
| `chart.2` | `Ansi 2 Color` |
| `chart.3` | `Ansi 3 Color` |
| `chart.4` | `Ansi 5 Color` |
| `chart.5` | `Ansi 1 Color` |
| `chart.6` | `Ansi 6 Color` |
| `chart.7` | `Ansi 12 Color` |
| `chart.8` | `Ansi 13 Color` |
| `window.background` | `Background Color` |

Tuned after the table (#2589). The terminal's own colours (its background, its words, its sixteen ANSI colours, its cursor and its selection) are what it draws, and are taken as they are:

| Token | Tuned |
|---|---|
| `selection.text` | when the theme's `selection.background` is filled and the table left it: `Foreground Color` |
| `text.muted` | toward `Foreground Color` |
| `syntax.comment` | toward `Foreground Color` |
| `danger.base` | toward `Foreground Color` |
| `accent.base` | toward `Foreground Color` |
| `focus.ring` | toward `Foreground Color` |
| `icon.red` | toward `Foreground Color` |
| `icon.orange` | toward `Foreground Color` |
| `icon.yellow` | toward `Foreground Color` |
| `icon.green` | toward `Foreground Color` |
| `icon.teal` | toward `Foreground Color` |
| `icon.blue` | toward `Foreground Color` |
| `icon.purple` | toward `Foreground Color` |
| `icon.pink` | toward `Foreground Color` |
| `state.running` | toward `Foreground Color` |
| `state.waiting` | toward `Foreground Color` |
| `state.failed` | toward `Foreground Color` |
| `state.success` | toward `Foreground Color` |
| `state.unreadable` | toward `Foreground Color` |
| `needs-you.base` | toward `Foreground Color` |
| `syntax.keyword` | toward `Foreground Color` |
| `syntax.string` | toward `Foreground Color` |
| `syntax.number` | toward `Foreground Color` |
| `syntax.constant` | toward `Foreground Color` |
| `syntax.function` | toward `Foreground Color` |
| `syntax.type` | toward `Foreground Color` |
| `syntax.property` | toward `Foreground Color` |
| `syntax.attribute` | toward `Foreground Color` |
| `syntax.tag` | toward `Foreground Color` |
| `syntax.heading` | toward `Foreground Color` |
| `syntax.invalid` | toward `Foreground Color` |
| `syntax.variable` | toward `Foreground Color` |
| `markdown.link` | toward `Foreground Color` |
| `markdown.code` | toward `Foreground Color` |
| `markdown.quote` | toward `Foreground Color` |
| `markdown.heading` | toward `Foreground Color` |
| `chart.1` | toward `Foreground Color` |
| `chart.2` | toward `Foreground Color` |
| `chart.3` | toward `Foreground Color` |
| `chart.4` | toward `Foreground Color` |
| `chart.5` | toward `Foreground Color` |
| `chart.6` | toward `Foreground Color` |
| `chart.7` | toward `Foreground Color` |
| `chart.8` | toward `Foreground Color` |

### Ghostty

| Token | Takes, nearest first |
|---|---|
| `surface.base` | `background` |
| `text.primary` | `foreground` |
| `text.muted` | `palette 8` |
| `accent.base` | `palette 4` |
| `focus.ring` | `palette 4` |
| `danger.base` | `palette 1` |
| `icon.red` | `palette 1` |
| `icon.yellow` | `palette 3` |
| `icon.green` | `palette 2` |
| `icon.teal` | `palette 6` |
| `icon.blue` | `palette 4` |
| `icon.purple` | `palette 5` |
| `state.running` | `palette 4` |
| `state.waiting` | `palette 3` |
| `state.failed` | `palette 1` |
| `state.success` | `palette 2` |
| `terminal.background` | `background` |
| `terminal.foreground` | `foreground` |
| `terminal.cursor` | `cursor-color` |
| `terminal.cursor-accent` | `cursor-text` |
| `terminal.selection` | `selection-background` |
| `terminal.selection-foreground` | `selection-foreground` |
| `terminal.ansi.black` | `palette 0` |
| `terminal.ansi.red` | `palette 1` |
| `terminal.ansi.green` | `palette 2` |
| `terminal.ansi.yellow` | `palette 3` |
| `terminal.ansi.blue` | `palette 4` |
| `terminal.ansi.magenta` | `palette 5` |
| `terminal.ansi.cyan` | `palette 6` |
| `terminal.ansi.white` | `palette 7` |
| `terminal.ansi.bright-black` | `palette 8` |
| `terminal.ansi.bright-red` | `palette 9` |
| `terminal.ansi.bright-green` | `palette 10` |
| `terminal.ansi.bright-yellow` | `palette 11` |
| `terminal.ansi.bright-blue` | `palette 12` |
| `terminal.ansi.bright-magenta` | `palette 13` |
| `terminal.ansi.bright-cyan` | `palette 14` |
| `terminal.ansi.bright-white` | `palette 15` |
| `syntax.keyword` | `palette 5` |
| `syntax.string` | `palette 2` |
| `syntax.number` | `palette 11` |
| `syntax.constant` | `palette 11` |
| `syntax.comment` | `palette 8` |
| `syntax.function` | `palette 4` |
| `syntax.type` | `palette 3` |
| `syntax.property` | `palette 6` |
| `syntax.tag` | `palette 1` |
| `syntax.heading` | `palette 4` |
| `syntax.invalid` | `palette 1` |
| `syntax.variable` | `palette 1` |
| `markdown.link` | `palette 4` |
| `markdown.code` | `palette 2` |
| `markdown.quote` | `palette 6` |
| `markdown.heading` | `palette 4` |
| `selection.background` | `selection-background` |
| `selection.text` | `selection-foreground` |
| `caret` | `cursor-color` |
| `chart.1` | `palette 4` |
| `chart.2` | `palette 2` |
| `chart.3` | `palette 3` |
| `chart.4` | `palette 5` |
| `chart.5` | `palette 1` |
| `chart.6` | `palette 6` |
| `chart.7` | `palette 12` |
| `chart.8` | `palette 13` |
| `window.background` | `background` |

Tuned as an iTerm2 preset is, after the table (#2589):

| Token | Tuned |
|---|---|
| `selection.text` | when the theme's `selection.background` is filled and the table left it: `foreground` |
| `text.muted` | toward `foreground` |
| `syntax.comment` | toward `foreground` |
| `danger.base` | toward `foreground` |
| `accent.base` | toward `foreground` |
| `focus.ring` | toward `foreground` |
| `icon.red` | toward `foreground` |
| `icon.orange` | toward `foreground` |
| `icon.yellow` | toward `foreground` |
| `icon.green` | toward `foreground` |
| `icon.teal` | toward `foreground` |
| `icon.blue` | toward `foreground` |
| `icon.purple` | toward `foreground` |
| `icon.pink` | toward `foreground` |
| `state.running` | toward `foreground` |
| `state.waiting` | toward `foreground` |
| `state.failed` | toward `foreground` |
| `state.success` | toward `foreground` |
| `state.unreadable` | toward `foreground` |
| `needs-you.base` | toward `foreground` |
| `syntax.keyword` | toward `foreground` |
| `syntax.string` | toward `foreground` |
| `syntax.number` | toward `foreground` |
| `syntax.constant` | toward `foreground` |
| `syntax.function` | toward `foreground` |
| `syntax.type` | toward `foreground` |
| `syntax.property` | toward `foreground` |
| `syntax.attribute` | toward `foreground` |
| `syntax.tag` | toward `foreground` |
| `syntax.heading` | toward `foreground` |
| `syntax.invalid` | toward `foreground` |
| `syntax.variable` | toward `foreground` |
| `markdown.link` | toward `foreground` |
| `markdown.code` | toward `foreground` |
| `markdown.quote` | toward `foreground` |
| `markdown.heading` | toward `foreground` |
| `chart.1` | toward `foreground` |
| `chart.2` | toward `foreground` |
| `chart.3` | toward `foreground` |
| `chart.4` | toward `foreground` |
| `chart.5` | toward `foreground` |
| `chart.6` | toward `foreground` |
| `chart.7` | toward `foreground` |
| `chart.8` | toward `foreground` |

## Exporting a theme

`purlis theme export <theme folder>` writes the folder as a zip to hand to someone, at
`--to <file.zip>` or as `<folder's name>.zip` here; a file there is replaced only with
`--force`. A level's theme folder is named by its id, as `purlis theme list` shows it and
`purlis theme check` takes it (`machine:<folder>`, `project:<folder>`,
`workspace:<workspace>:<folder>`): the engine finds it at that level alone, as a choice made
there finds it, and a level with no folder of that name is refused, naming the level's form,
never taking a farther level's folder in its place. `purlis theme export purlis-classic` writes Purlis Classic's own `theme.json` as the
folder `purlis-classic-copy`, which imports beside the built-in, never in its place.

```
exported harbor (1 file) to harbor.zip
```

The zip holds one folder, named as the theme folder is, so unzipping it gives the folder back;
`purlis theme import harbor.zip` gives back the same files, byte for byte. It carries
`theme.json`, every plain file beside it (a licence, a preview), and the plain files in `fonts/`
and `icons/`. What it does not carry, a hidden file, a link or another folder, is listed under
`not carried`. The same folder always makes the same zip.

Importing a zip reads only what an export writes: one folder, with files beside `theme.json` and
in its `fonts/` and `icons/`; what the macOS Finder adds to a zip it makes (`__MACOSX/`,
`.DS_Store`) is passed over and not counted. A zip with bytes after its end is refused. A zip with an entry that climbs out of the folder, starts at the
root, is a link or is encrypted, a folder that is not a theme folder's id, two names a
filesystem writes as one file (in other case or other accents), and a zip whose end lists more
entries than that are refused whole, and nothing of it is written. A zip holds at
most 64 files, each at most 2 MiB (the most a font may be) and 16 MiB together.

## Design tokens

`theme.json` is the only format purlis reads a theme in. The W3C Design Tokens Community
Group's format, version 2025.10, is a way in and out of it, so a design tool and purlis can
exchange a theme's tokens: `purlis theme export <theme folder> --kind design-tokens` writes the
folder as one design-tokens file, at `--to <file>` or as `<folder's name>.tokens.json` here, and
`purlis theme import <file>.tokens.json` reads one back. **Exporting and then importing gives
the same theme folder**: the same `theme.json`, its component parts and grey tiers included, and
the same files beside it. Every built-in theme does, and the export is checked against the
format's official schema in CI.

```
exported purlis-copy (12 files) to purlis-copy.tokens.json
```

What is a token:

- **The palette** is the group `palette`, a `color` token for each colour.
- **Each appearance** is the group `appearance.<id>`. A token's name is split at its dots into
  groups (`text.muted` is `text` › `muted`). A colour that names a palette colour is a reference
  to it (`{palette.label}`); one given as hex is the colour. A geometry token is a `dimension` in
  `px`, a font weight a `fontWeight`, a line height a `number`. The motion is the group
  `appearance.<id>.motion`: each duration a `duration` in `ms`, each easing a `cubicBezier`.
- **The `compact` layer** is the group `compact`, typed as an appearance's geometry is.
- A name that is a token and also starts other tokens' names is its group's `$root`.

What the format has no type for (a flag, a word such as `platform` or the cursor's shape, a
component part, an option, the tint, the fonts and icon set a theme names) is kept as
`theme.json` writes it under the vendor key `dev.purlis.theme` in `$extensions`, which the
format asks every tool to keep: on an appearance's group, its tag, `high-contrast` and its
untyped values; on the file, the rest of `theme.json`. The files beside `theme.json` (a font, a
licence, an icon set) travel there too, as base64.

**Colours are hex.** A colour is written in `srgb` with its `hex`, and its `alpha` when it is not
opaque. Reading, purlis takes a colour's `srgb` components or, in any other colour space or when
a component is `none`, its `hex`; a colour with neither, and one whose components and hex are two
colours, refuse the file with a sentence that names the token. A colour spelled another way than the format writes it
(`#FFF`) keeps its spelling under the vendor key.

**A file purlis did not write** is read by the same rules. Its `appearance.<id>` groups are the
appearances; with none, the tokens at its top that purlis has names for (`surface.base`,
`text.primary`) are one appearance, dark or light by its ground. A reference to its `palette`
keeps the palette name, and any other reference is followed to the colour it ends at. A token
no purlis token takes, a dimension in `rem`, a font weight given as a word, a JSON pointer
(`$ref`) and a group's `$extends` are listed under `not taken`. A reference to a token that is
not there, or round in a circle, refuses the file.

Imports are bounded in size and work. A design-tokens file is at most 24 MiB and holds at most
400,000 JSON values, counted before it is read, no number longer than 64 KiB and no more strings
that long than the files it may carry; a number is taken when it is at most 32 characters long;
a group's or a token's name is at most 64 bytes
and a token's path, its groups' names and its own, at most 256; the `theme.json` it makes is at
most 2 MiB, as a zip's is; it has at most 64 appearances; the files it carries are held to a
zip's bounds (64 files, each at most 2 MiB, 16 MiB together, each where a theme folder keeps a
file, no two a filesystem writes as one); its groups nest at most 32 deep and it holds at most
50,000 groups and tokens. A file a newer purlis wrote, whose `dev.purlis.theme` version this one does not
read, and one whose version is not a whole number, are refused with the version they have.
A `.json` file with tokens in it is read as design tokens even when its top group is `colors`.

## Names every platform holds

A theme made on one platform opens on every other, so on every platform purlis refuses a theme
folder's name, and the name of a file in it, that Windows cannot hold: a device name (`CON`,
`PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `COM0` to `COM9`, `LPT0` to `LPT9`, in any case, alone or before an
extension, as `nul.json`), or a name ending in a dot or a space. Each refusal is one sentence that
names the name. An import of a zip naming one is refused whole; a theme whose name would make
such a folder is not imported; an export leaves such a file out and lists it under
`not carried`; a level's folder named so is listed as refused, and a font or an icon set named
so refuses the theme.

## An extension's theme

An extension's theme is read and checked like any other. Why the engine refused it, or what it
left out, is shown on the extension's row in Extensions, beside its other complaints, and
`purlis theme check <extension>/<theme>` reports it in full.
