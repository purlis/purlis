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

Each theme's `id` is what the other commands take: `purlis-classic`, `purlis`,
`machine:<folder>`, `project:<folder>`, `workspace:<workspace>:<folder>`, `<extension>/<theme>`.
`theme_schema` is the address a new `theme.json` names in its `$schema`.

Choose the theme folder's name: lower-case letters, digits and dashes, starting with a letter,
at most 32. Editing a workspace's or the project's theme, use its folder's name; a new theme
takes a name no folder here has. Then:

```bash
purlis theme draft <folder>
```

It starts the draft (a copy of the workspace's or the project's folder of that name, or a new
theme that extends Purlis Classic) and prints the working copy's `theme.json`. Done when it
printed that path.

## 2. Write the theme

Work in the working copy's `theme.json`. Purlis Classic's own file lists every token with its
value; read it from a copy:

```bash
purlis theme export purlis-classic --to "$TMPDIR/purlis-classic.zip"
unzip -o "$TMPDIR/purlis-classic.zip" -d "$TMPDIR/purlis-classic"
```

The format and the check are documented in full: `purlis docs show themes`. Keep it small:
`"extends": "purlis-classic"` and only what differs. Put each colour in the `palette` once and
name it from the tokens. Most of a look is `surface.*`, `text.*`, `border.*`, `accent.base`,
`accent.surface`, `focus.ring`, `tab.active`, `list.selected`, `list.selected-edge` and the
`terminal.*` colours. A theme with a `dark` and a `light` appearance follows the system.

Start from what the person gave you:

- **A brand colour**: make it the palette's accent and draw `accent.base`, `focus.ring`,
  `list.selected-edge`, `markdown.link` and `chart.1` from it; mix `accent.surface`, `tab.active`
  and `list.selected` from it and the surface. Give the light appearance a darker step of the
  same hue, so it reads on a light surface.
- **An image or a screenshot**: read it, and take the background, the raised surfaces, the
  words, the rules and the accent from its own colours, as hex.
- **A Figma frame**: read the frame's variables and colours with the Figma tools this chat has;
  without them, ask the person for a screenshot of the frame and work from that.
- **An imported theme** (a VS Code colour theme, a base16 or base24 scheme, an iTerm2 or a
  Ghostty palette, a theme folder's zip): import it into a folder of your own, then make that
  folder the working copy:

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

Done when the file is JSON with every value hex, a palette name, or a token's own type.

## 3. Draw, look, check, fix

Loop until the check is clean:

```bash
purlis theme draft <folder> --save
```

The app writes the draft and says what the window draws. Exit 1 means it is not drawn: it
breaks a hard floor (primary text, the focus ring, the terminal's words, each at 3:1), and the
answer names the pair and its ratio. Fix that first.

Look at it with purlis's `theme_preview` tool: the theme specimen, in each appearance, side by
side with the theme it starts from. It shows sample content only. A purlis whose
`theme_preview` takes only its built-in themes cannot picture the draft yet; then look at the
theme it extends, judge the draft by the check, and ask the person how it looks in their window,
where it is drawn.

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

- **With the team**: a project's theme is committed by the plane's next save.
