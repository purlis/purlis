# A theme is bounded data and may set every token

**Accepted 2026-10-16**, from the operator's spec #2313 (M64, Themes), ticket #2314.
**Supersedes [ADR 0094](0094-themes-own-colour-and-motion-and-core-owns-geometry.md)**: its rule
that a theme owns colour and motion and core owns geometry. What 0094 built on that rule, the
token layers, the control baseline and one class per control (its §1, §3 and §4), stands, as
said below. It keeps [ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)'s
four properties of a declarative extension, changes how one of its three crossings is held, and
amends its rule that the consent surfaces and the state colours are purlis's (the operator's
ruling of 2026-10-17).

## Where purlis was

ADR 0094 drew the safety line around a theme by what it may name: colour, the shadow's colour and
motion, and nothing else. Geometry, fonts, the icon set and how a workspace's tint lands were
core's. That kept a theme from moving a control's height, and it also kept a designer's look out
of reach: a denser layout, softer corners or another typeface could not be written as a theme at
all. A theme also had one appearance, so a custom theme could never be the light half of
anything.

## The decision

### 1. A theme may set every token

**Every token in the published theme vocabulary may be set by any theme**: colour, motion,
geometry, the UI, mono and terminal fonts, its icon set, which tokens a workspace's tint changes,
the tokens of the consent surfaces and the state marks, and the component tokens. Core still gives
every token its default value, which is Purlis Classic's, so a theme holds only what it changes.

**Component tokens are in the vocabulary.** Each one defaults to the semantic token it aliases
today (`--btn-bg` to `control.base`), so a theme that sets none draws every control as the
semantic tokens say, and one that sets a component token is held to the same bounds and hard
floors as any other.

The same vocabulary and the same bounds apply to every theme, whatever its source: built in, the
person's own, a project's, a workspace's or an extension's.

### 2. The bound is on the value, not on the name

**Every value a theme gives is typed and bounded**, and that is the safety boundary in place of
"colour and motion only":

- a colour is hex, or the name of a colour in the theme's own palette: a closed grammar, never
  CSS;
- a geometry value is a number inside the range core documents for that token;
- motion keeps its closed grammar;
- a flag is `true` or `false`, and a choice is one of the names core lists for that token (the
  syntax roles' italic and bold, the terminal cursor's shape, #2327);
- a font is a woff2 file inside the theme folder, under a size cap, served by purlis, never a URL;
- an icon keeps today's path grammar and is drawn as elements, never as SVG text.

A value outside its type or its bound refuses the theme, which is drawn as Purlis with a Notice
that says what is wrong. A contrast failure is a warning: the theme's author decides what is good
enough for them. The one exception is the consent floors (§6), which hold for every theme.

### 3. ADR 0041's four properties hold

1. **The vocabulary is closed and purlis decides it.** A theme fills in values for the tokens
   purlis publishes; the schema is generated from the engine's own definition.
2. **The value space is not a program.** A colour, a bounded number, a member of an enumeration,
   a path outline, a font file.
3. **purlis chooses the consumer.** A theme never says where a token lands.
4. **purlis parses and re-emits.** A value is read into a typed value and written out fresh; a
   theme's bytes never reach a stylesheet.

**The third crossing, "a reference to a file", is held by the folder.** A font or an icon set is
named relative to the theme folder and read only from inside it. A path that leaves the folder,
a link, or a URL is not a value the grammar has, so it refuses the theme. purlis reads the file
and serves it to the window itself.

### 4. A theme is a folder, with appearances and options

A theme is a **theme folder**: a `theme.json` with a published schema, and optionally its fonts,
its icons, a preview and a licence. It may `extend` another theme and hold only its differences.
It has a **palette** of named colours, one or more **appearances**, each tagged dark or light and
optionally high contrast, and **theme options** (toggles and choices), each a layer of token
values. Options are token-only: no option is a free number. Every appearance and option
combination is checked, up to 64 of them; a theme with more is refused with its count.

### 5. One engine owns a theme, and the window applies it

One theme engine in the Rust core reads, validates, resolves, checks, imports and exports a
theme, and every surface calls it: the window, the CLI, the skill's specimen and a draft's hard
floors. The window receives a resolved theme and its complaints and draws them.

### 6. What holds whatever a theme says

- **Hard floors**: primary text on every surface at 3:1 or more and a focus ring visible against
  its surfaces, plus the short list the engine keeps. A theme draft that breaks one is never
  drawn.
- **The consent surfaces and the state marks are in the vocabulary, under hard floors of their
  own.** The consent surfaces are the asks, Allow and Keep blocked, and needs-you. Beside the
  general floors, the engine's list holds that an ask contrasts with its surface, that Allow is
  distinguishable from Keep blocked, and that danger is distinguishable from primary. These are
  the consent floors.
- **The consent floors hold for every theme**, in every combination of its appearances and
  option values: when a theme's consent-surface or state-mark colours break a consent floor, the
  engine draws purlis's own values for those tokens, with a Notice naming the floor; the rest of
  the theme draws as written. The floors are held after the option values are layered, so no
  option value can bring a consent surface below its floor.
  Contrast between other tokens stays a warning.
- **A consent surface's wording, icons and placement are purlis's**, and are not theme data. A
  theme colours a prompt; it cannot draw one.
- **Density, text size, the font override and reduced motion** are the person's settings, and
  every theme honours them. A theme may tune what compact means for it; it cannot turn compact
  off.

### 7. What stands from ADR 0094

- The three token layers. A theme's palette is the primitive layer, its tokens are the semantic
  layer, and component tokens are the aliases inside the control classes, now in the vocabulary
  (§1).
- The control baseline: every control takes the window's font and its height from padding.
- One CSS class per control kind, over Radix.
- Lengths that grow with the words are in rem.

Until the engine is built, the window keeps holding 0094's rule, and geometry stays core's; this
record is what the M64 tickets build to.

## What was rejected

- **Keeping geometry out of themes.** The look the operator wants as the new default cannot be
  expressed without it, and a bound on each value keeps alignment as safe as a closed name list
  did.
- **A blocklist of CSS for richer values.** A theme's values stay grammars purlis parses, as
  ADR 0041 requires; a blocklist is the version of that rule that fails.
- **Fonts and icons by URL.** A fetch is a capability, not data.
- **Free-number options (sliders).** They make the combinations uncheckable.

## Consequences

- A theme can change the window's sizes, type and icons, within documented bounds.
- Every value a theme gives needs a documented bound and a test at the engine's interface.
- The contrast suite moves into the engine so it can run on any theme.
- `docs/design-system.md` describes the tokens and their bounds as the M64 tickets build them.

The amendments to this decision are files of their own in [`0097/`](0097/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record, so two pull requests that amend it do not touch the same lines
(#1982).
