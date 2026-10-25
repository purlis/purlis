---
name: theme
description: Create, edit, check or share a purlis theme — from a brand colour, an image or screenshot, a Figma frame, an imported theme or another theme — through a theme draft the window draws live, checked until it is clean. Use when asked to make, change, fix, check, export or share a theme, or to make purlis look a certain way.
---

# Making a theme

A **theme** is how the purlis window looks: a theme folder whose `theme.json` holds a palette,
one or more appearances (dark, light, high contrast) and their token values. You write a
**theme draft**: a copy of the theme that the window draws live, only where this chat works,
while the person watches. The person decides what to keep, with **Keep**, **Revert** and
**Compare** on the strip the draft puts on the workspace.

**Where the theme goes is where this chat works.** Started in a workspace, the draft is that
workspace's, and Keep puts it in the workspace's theme folder of that name, else the project's.
Started at the plane root, it is the project's. The person chooses the level by where they open
the chat. A read-only project keeps no theme: a draft there can be drawn, checked and exported,
and Keep is refused. When the project is read-only, tell the person that before you start.

**What you write is the working copy.** `purlis theme draft` makes it in your temporary folder
and hands it to the app, and the app writes the draft. Write only there and in folders of your
own under `$TMPDIR`. The theme folders change when the person presses Keep, and `.purlis/` is
purlis's own: every change you make reaches them through `purlis theme draft --save`. A prompt
or an approval the window shows is the person's to answer.

## 1. Start the draft

Read what is here:

```bash
purlis theme list --json
```

Each theme's `id` is what the other commands take: `purlis` (Purlis, the default theme),
`purlis-classic` (Purlis Classic, the alternative), `machine:<folder>`, `project:<folder>`,
`workspace:<workspace>:<folder>`, `<extension>/<theme>`.
`theme_schema` is the address a new `theme.json` names in its `$schema`.

Choose the theme folder's name: lower-case letters, digits and dashes, starting with a letter,
at most 32. Editing a workspace's or the project's theme, use its folder's name; a new theme
takes a name no folder here has. Then:

```bash
purlis theme draft <folder>
```

It starts the draft (a copy of the workspace's or the project's folder of that name, or a new
theme that extends Purlis, the default) and prints the working copy's `theme.json`. Done when it
printed that path.

## 2. Write the theme

Work in the working copy's `theme.json`. Purlis's own file lists every token of its three
appearances, from the colours of its `palette`; read it from a copy:

```bash
purlis theme export purlis --to "$TMPDIR/purlis.zip"
unzip -o "$TMPDIR/purlis.zip" -d "$TMPDIR/purlis"
```

The format and the check are documented in full: `purlis docs show themes`. Keep it small:
`"extends": "purlis"` and only what differs. When the person wants Purlis Classic's look as the
start, extend `purlis-classic` instead, and read its file the same way. Put each colour in the
`palette` once and name it from the tokens. Most of a look is `surface.*`, `text.*`,
`border.*`, `accent.base`, `accent.surface`, `focus.ring`, `layer.selected`, `list.selected`,
`list.selected-edge`, the `terminal.*` colours and its shape (below). Over Purlis, its filled
primary is `accent.fill` (`accent.fill-hover` under the pointer, `accent.fill-edge` its edge)
with `accent.fill-text` words, held to 4.5:1; over Purlis Classic, a theme that leaves these out fills its primary with its
`accent.surface`, and `border.control` (a plain control's edge) follows its `border.strong`. A theme with a `dark` and
a `light` appearance follows the system.

**The selected tab is `layer.selected`**, on all three strips; keep it apart from
`list.selected`. `tab.active` is not a tab: it colours menu highlights and button hovers.

Start from what the person gave you:

- **A brand colour**: make it the palette's accent and draw `accent.base`, `focus.ring`,
  `list.selected-edge`, `markdown.link` and `chart.1` from it; mix `accent.surface`,
  `list.selected` and `layer.selected` from it and the surface. Give the light appearance a
  darker step of the same hue, so it reads on a light surface.
- **An image or a screenshot**: read it, and take the background, the raised surfaces, the
  words, the rules and the accent from its own colours, as hex.
- **A Figma frame**: read the frame's variables and colours with the Figma tools this chat has;
  without them, ask the person for a screenshot of the frame and work from that.
- **An imported theme** (a VS Code colour theme, a base16 or base24 scheme, an iTerm2 or a
  Ghostty palette, a theme folder's zip, a W3C design-tokens file from a design tool): import
  it into a folder of your own, then make that folder the working copy:

  ```bash
  purlis theme import <file> --into "$TMPDIR/purlis-theme-import"
  purlis theme draft <folder> --from "$TMPDIR/purlis-theme-import/<folder>"
  ```

  Give `--from` to every later `purlis theme draft` of it too. An imported theme's fonts stay in
  that folder: a draft takes theme files only.
- **Another theme**: a workspace's or the project's theme folder is extended by its folder's
  name. For one of another level, read the `theme.json` at the `path` `purlis theme list --json`
  gives it and copy what you need into the working copy.

Editing a theme of the machine's level, a built-in or an extension's makes a new theme of this
workspace or project, and the original stays as it is: say so to the person.

A draft brings no font. A face is drawn only when the project approved that file in the theme's
folder, and the window shows the person a Notice for a face it refuses. Tell the person about
each font you named that their folder does not already have.

**Shape and size** are tokens beside the colours, set in each appearance (one that leaves them out
keeps the theme it extends): a whole number (the line heights take decimals) inside its bounds, or
the theme is refused. Offer them: the built-in Purlis theme sets `radius.sm` and `radius.md` 7,
`radius.control` 8, `radius.lg` 10, `radius.xl` 12, `size.control` 28, `size.chrome` 24,
`font-size.base` 13, `line-height.body` 1.55, `window.gutter` 8 and `panel.corner-radius` 12 in
all three of its appearances: its panels float with rounded corners,
where Purlis Classic draws them flush. The top-level `compact` holds what compact density draws. A token it leaves out keeps
the appearance's own value, except the ones Classic's compact moves (`space.2` to `space.8`,
`line-height.body`, `size.control`, `size.chrome`): those take the smaller of Classic's compact and
the appearance's own.

Most asks for another shape come down to the corners and the heights:

| Token | Bounds | Classic | Compact | What it reaches |
|---|---|---|---|---|
| `radius.sm` | 0–8 px | 3 | 3 | labels, chips, crumbs, badges, checkboxes, error and report boxes |
| `radius.md` | 0–10 px | 4 | 4 | menu rows, Notices, the palette's rows, Inbox asks, a tab's hover pill |
| `radius.control` | 0–14 px | 4 | 4 | an md control: buttons, fields, selects, a list's rows, title-bar chips, a tab's top |
| `radius.lg` | 0–14 px | 6 | 6 | dialogs, popovers, menus, tooltips, segmented controls |
| `radius.xl` | 0–18 px | 8 | 8 | the Welcome page's doors, nothing else |
| `radius.full` | 0–9999 px | 9999 | 9999 | pills: counts, toggle tracks |
| `size.control` | 20–40 | 28 | 24 | buttons, fields, selects, menu rows, a list's rows, the workspace strip |
| `size.chrome` | 16–32 | 21 | 18 | small buttons, the chat strip, pane and panel headers, a dense list's rows and every tree row |
| `window.gutter` | 0–16 px | 0 | 0 | the gap between the window's regions: 0 draws them flush, 8 floats them |
| `panel.corner-radius` | 0–24 px | 0 | 0 | the corners of the side panels, the content and the bottom panel's halves |
| `panel.border-width` | 0–2 px | 1 | 1 | the hairline between two regions that sit flush |

`purlis docs show themes` has every one in its Geometry section: the spacing and type scales,
the weights, the line heights, the focus ring and the shadows too, each with its unit, bounds,
values and what it reaches. Read it before you set one this table leaves out. The window's own
text size is the person's text-size setting, which no theme sets: point them at it when they
ask for bigger words.

Sizes are px at the default text size and grow with it; corners and the focus ring do not. The
project strip sits in the title bar, whose height no token reaches yet.

A line must fit its box, in both densities: `font-size.base` × `line-height.body` within
`size.control`, and `font-size.md` × `line-height.body` within `size.chrome`. Compact is
tighter: `font-size.md` 14 fits comfortable (14 × 1.4 = 19.6, within 21) and fails compact
(14 × 1.3 = 18.2, over 18). The check reports each line that does not fit, with its density.

**Still square, whatever `radius.*` says:** the project and workspace strip tabs, the settings
gear on a strip, and every tree row. Tell the person when they ask for rounder ones: the theme
cannot round those yet. The session tabs (chats, views, files) and their `×` are rounded through
their parts (`tabs.*`, below), and a list's rows by `radius.control` and the `list` family's parts. The
focus ring tokens reach every focus ring, the strip tabs', the closer's and the gear's among them.

**One part of one control** is a component part, in a top-level `parts` object:
`"button.root.corner-radius": "full"` rounds every button to a pill, and
`"button.root.primary.hover.fill": "accent.base"` changes only the primary under the pointer. Run
`purlis theme parts` for every family, part, variant, state and property with its bounds before
you write one. A part's colour names a semantic token (`accent.base`), never a hex or a palette
name. Inside a state (`hover`, `pressed`, `selected`, `focused`, `disabled`, `open`) only `fill`,
`text`, `border-colour` and `shadow` may be set: a state never changes size, and the theme is
refused if it tries. A button stays at least 24px tall with its words fitting, in both densities,
and a geometry token is held to the same bounds at the value the theme draws it at. Whatever the
theme changes, Allow (`grants`) keeps its accent edge and Keep blocked's fill, and no part draws
Allow or danger nearer the primary than the family does: a look that would is drawn as the
family default. The button, the fields, the tooltip and the dialog have parts so far: `field`
(its label, help and error), `text-input` (the one-line box and its glass), `textarea` and
`tooltip` (its `root`, the surface: colours, edge, corners and shadow, no sizes; and `detail`, an
icon button's second line), which have no variants, so a key names the part alone
(`"text-input.root.border-width": 0` turns off every one-line box's edge); and `dialog`
(`dialog.root.corner-radius`, `dialog.title.font-size`, `dialog.separator.fill`, …), whose size
and rows are never a theme's, whose corner is drawn at most `space.6`, and whose quieter lines
are held to 4.5:1 in their own tiers; and `list` (`list.root.corner-radius`,
`list.root.selected.fill`, `list.body.padding`, …), a list's row, which has no variants and whose
dense and large sizes are never a theme's. The note above still holds for the strip tabs and tree
rows.

**The session tabs** are the `tabs` family. Purlis docks them on the content panel: the chosen
tab takes the panel's fill and flares into it, an unchosen tab's hover is a pill inset from its
cell. `"tabs.layout.attach": "strip"` puts them on a strip of their own (Purlis Classic's look),
`tabs.layout.sizing` is `content` or `equal`, `tabs.layout.dividers` `on` or `off`,
`tabs.layout.gap` the room between them; `tabs.tab.selected.flare` (px, 0 for none) is set only
on the selected state and `tabs.tab.hover.inset` (one value, or top, sides, bottom) only on hover:
both are drawn out of the tab's flow, so they move nothing. A heavier chosen tab
(`tabs.tab.selected.font-weight`) is refused like every size inside a state. `tabs.indicator.*`
draws a mark on the chosen tab's `top` or `bottom` edge, at most 6px tall. A file's tab joins the
editor's toolbar (`editor.toolbar.fill`).

Done when the file is JSON with every value hex, a palette name, or a token's own type, and
every shape and size inside its bounds.

## 3. Draw, look, check, fix

Loop until the check is clean:

```bash
purlis theme draft <folder> --save
```

The app writes the draft and says what the window draws. Exit 1 means it is not drawn: it
breaks a hard floor (primary text, the focus ring, the terminal's words, each at 3:1), and the
answer names the pair and its ratio. Fix that first.

Look at it with purlis's `theme_preview` tool: name the draft as `draft:<folder>` and draw the
theme specimen in each appearance, with each option value the theme has, side by side with the
theme as kept (`side_by_side: {"theme": "<folder>"}`). A new theme has no kept folder yet: put it
beside the theme it extends (`side_by_side: {"theme": "purlis"}`). It shows sample
content only.

```bash
purlis theme check draft:<folder> --json
```

Fix what it reports, then save and check again:

- `errors` and `refused`: a value the grammar refuses; the theme is not drawn.
- `notes`: a name purlis does not have, left out.
- `warnings`: contrast, each `pair` with its `ratio` and `floor`, per appearance and option
  value and per workspace tint. Move the front colour along its own hue until it reads.
- `held`: a consent floor the window holds with purlis's own colours.
- `hard_floors`: a floor that keeps the draft from being drawn.

Done when `clean` is true and `hard_floors` is `[]`, or each warning left is one you can name
with why it stays (the person's brand colour itself, say).

## 4. Hand it over

Tell the person what you made, what the check found, and each warning left with why. Keep,
Revert and Compare are on the workspace's strip; Keep asks them once whether the theme becomes
the workspace's (or the project's) default or stays their own choice. Done when the person has
the check's result in front of them.

## Checking a theme

```bash
purlis theme check <id> --json
```

Any id from the list, or the path of a theme file or folder. It writes nothing. Exit 0: drawn;
1: refused (or a draft that breaks a hard floor); 2: no such theme. Report it as in step 3.

## Sharing a theme

- **Up a level**: in Settings › Theme, **Use in the whole project** moves a workspace's theme to
  the project, and **Copy to my machine** copies a project's or a workspace's to the person's
  own. Point the person at them.
- **As a file**: export a theme folder as a zip that `purlis theme import` gives back, into a
  folder of your own, and tell the person where it is:

  ```bash
  purlis theme export <id> --to "$TMPDIR/<folder>.zip"
  ```

- **To a design tool**: `--kind design-tokens` writes the folder as one W3C design-tokens file
  (format 2025.10) instead, which `purlis theme import` also gives back the same:

  ```bash
  purlis theme export <id> --kind design-tokens --to "$TMPDIR/<folder>.tokens.json"
  ```

- **With the team**: a project's theme is committed by the plane's next save.
