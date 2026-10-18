# Themes

A **theme** is how the window looks: every colour and every motion value, in each of its
appearances (dark, light, high contrast). purlis's theme engine reads each theme, refuses one
that breaks its grammar, decides which theme the window draws, and checks whether its words can
be read. The window only draws what the engine hands it. The full format is in the
repository's `docs/design-system.md`, and its schema is `docs/schema/theme.v2.schema.json`.

## Where a theme comes from

- **Built in**: Purlis Classic, `purlis-classic`, with a dark and a light appearance. The ids
  its appearances have had (`purlis-dark`, `purlis-light`, `charter-dark`, `charter-light`)
  still name it.
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
plane there are no project or workspace levels.

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

An option nothing chose draws its default. The check covers **every combination** of
appearance and option values, so a theme has at most 64: the appearances times each option's
number of values (a toggle has two). A theme past that is refused with its count.

## The theme check

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
  text.muted on surface.hover, in the red, blue tints: down to 2.07, under 3
```

Then `floors held:`, what the window draws of the theme: each consent floor it breaks in a
combination, and the tokens the window draws as purlis's own for it (`danger is
distinguishable from primary — purlis draws its own danger.base`), or `none`. A last `hard
floors:` line follows. Until purlis keeps hard floors (#2332), it says they are not checked by
this purlis yet.

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
- **Floors held** are the consent floors (an ask contrasts with its surface, Allow is
  distinguishable from Keep blocked, the danger colours contrast with what they are drawn on,
  danger is distinguishable from primary, the state marks meet the mark floor). A theme that
  breaks one is drawn with purlis's own values for those tokens and the rest as written.
- **Hard floors** are the few rules a theme can never break. This purlis does not check them
  yet (#2332), and says so rather than reporting a pass.

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
  "hard_floors": null,
  "clean": false
}
```

`refused` is `null`, or why the theme is not drawn, and `errors` then lists each value that
refused it (`said`, and `token` when it was about one); `name` and `appearances` are `null`.
`notes` is what was left out of a theme that is drawn. A warning's `appearance` is the id of the
appearance it was found in, and its `options` the value of each option it was drawn with (`{}`
for a theme with none), in the theme's order of its options. Its `kind` is `contrast` (a `pair`,
its `ratio`, and `under` when the back is a wash laid over another token), `ansi` (a terminal
colour too close to the terminal's background: `token`, `ratio`, `floor`) or
`selected-not-apart` (the selected row drawn exactly as `other`). `tint` is the workspace colour
it was found in, or `null` for the theme as it is. `held` has one entry for each combination
in which the window holds a consent floor: its `appearance` and `options` as a warning's, the
`floors` (`ask`, `allow-apart`, `danger`, `danger-apart`, `state-marks`) and the `tokens` drawn
as purlis's own. `hard_floors` is `null` until purlis checks them. `clean` is true with no
error, no note, no warning and no floor held; it covers those only, never hard floors, which
`hard_floors` reports on its own. `schema` is the version of this shape, in both
answers: a reader checks it first. A theme folder's `from` is `{"kind": "folder", "path"}`, a
theme file's `{"kind": "file", "path"}`.

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

## An extension's theme

An extension's theme is read and checked like any other. Why the engine refused it, or what it
left out, is shown on the extension's row in Extensions, beside its other complaints, and
`purlis theme check <extension>/<theme>` reports it in full.
