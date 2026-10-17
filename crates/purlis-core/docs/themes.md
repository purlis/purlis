# Themes

A **theme** is how the window looks: every colour and every motion value, read from a file
purlis holds to a closed grammar. purlis's theme engine reads each theme, puts right what it can
and says so, decides which theme the window draws, and checks whether its words can be read. The
window only draws what the engine hands it. The full format is in the repository's
`docs/design-system.md`.

## Where a theme comes from

- **Built in**: `purlis-dark` and `purlis-light`, compiled into purlis (`charter-dark` and
  `charter-light` still name them).
- **Your own**: `theme.json` in purlis's config folder (`$PURLIS_CONFIG_HOME`, else
  `$XDG_CONFIG_HOME`, else `~/.config`, then `purlis/`). Its id is `mine`.
- **An extension**: each theme an approved extension contributes while it is on. Its id is
  `<extension>/<theme>`, as a project's pick names it.

`purlis theme list` shows every theme available here:

```
  ID                        NAME            APPEARANCE  FROM
  purlis-dark               purlis-dark     dark        built-in
  purlis-light              purlis-light    light       built-in
  mine                      Late            dark        /home/me/.config/purlis/theme.json
  solarized/Solarized Dark  Solarized Dark  dark        extension solarized
```

An id may hold a space, as an extension's theme name can: quote it on the command line
(`purlis theme check "solarized/Solarized Dark"`).

Then `refused:`, one line per theme purlis could not read and why. `--json` answers
`{"schema": 1, "themes": [{"id", "name", "appearance", "from", "refused"}]}`, where `from` is
`{"kind": "built-in"}`, `{"kind": "mine", "path"}` or `{"kind": "extension", "extension"}`,
and `refused` is `null` or why.

## The theme check

`purlis theme check <theme>` checks one theme, named by its id or by the path of a theme file:

```
Dim — /home/me/dim.json, dark
errors (1):
  text.primry is not a purlis token
warnings (2):
  text.muted on surface.base: 2.38, under 4.5; in every workspace tint too, down to 2.38
  text.muted on surface.hover, in the red, blue tints: down to 2.07, under 3
```

A last `hard floors:` line follows. Until purlis keeps hard floors (#2332), it says they are
not checked by this purlis yet.

- **Errors** are values purlis had to put right: a token that is not one, a value that is not
  hex, a duration out of range. Each is drawn as the built-in of the theme's appearance, and the
  window says so in the Inbox.
- **Warnings** are contrast findings: each pair of tokens that is drawn as text on a surface,
  with its ratio and the floor it is under, as the theme is and tinted by every workspace
  colour, one line per pair. A warning is the author's call; the theme is drawn.
- **Hard floors** are the few rules a theme can never break. This purlis does not check them
  yet (#2332), and says so rather than reporting a pass.
- A file that is not a theme at all (not JSON, not an object, too large) is **refused**. When it
  is your own `theme.json`, the window draws Purlis in its place and the Inbox names the
  problem, with **Use built-in** to move the file aside.

The exit is a linter's: **0** when the theme has no errors (warnings or not), **1** when it has
an error or is refused, **2** when there is no such theme.

`--json` is the same check for a chat:

```json
{
  "schema": 1,
  "theme": {"id": "mine", "name": "Dim", "appearance": "dark",
            "from": {"kind": "mine", "path": "/home/me/.config/purlis/theme.json"}},
  "refused": null,
  "errors": [{"said": "text.primry is not a purlis token", "token": "text.primry"}],
  "warnings": [{"kind": "contrast", "tint": null,
                "pair": {"front": "text.muted", "back": "surface.base", "floor": 4.5},
                "ratio": 2.38}],
  "hard_floors": null,
  "clean": false
}
```

A warning's `kind` is `contrast` (a `pair`, its `ratio`, and `under` when the back is a wash
laid over another token), `ansi` (a terminal colour too close to the terminal's background:
`token`, `ratio`, `floor`) or `selected-not-apart` (the selected row drawn exactly as `other`).
`tint` is the workspace colour it was found in, or `null` for the theme as it is.
`hard_floors` is `null` until purlis checks them. `clean` is true with no error and no warning;
it covers errors and warnings only, never hard floors, which `hard_floors` reports on its own.
`schema` is the version of this shape, in both answers: a reader checks it first.

## An extension's theme

An extension's theme is read and checked like any other. What the engine had to put right in
it is shown on the extension's row in Extensions, beside its other complaints, and
`purlis theme check <extension>/<theme>` reports it in full.
