# A theme sets component parts through a closed manifest, and a state never changes size

**Accepted 2026-10-21**, from the operator's spec #2897 (M66, UI system) and its resolutions
#2794 (what a part is), #2795 (how tokens flow to the window) and #2800 (a state never changes
size), all accepted as recommended on 2026-10-20 and 2026-10-21; ticket #2903. It extends
[ADR 0097](0097-a-theme-is-bounded-data-and-may-set-every-token.md): a theme stays bounded data,
and gains a second layer beside its tokens. It keeps
[ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)'s four properties of a
declarative extension.

**Built by #2903**, with the button as its first family.

## Where purlis was

A theme could set every token (ADR 0097), and a token reaches every control drawn from it.
`radius.md` rounds every button, field, select and menu row at once, within a cap that keeps a
field a field. Nothing could say "this part of this control": round the button to a pill and
leave the fields alone, turn a control's border off, change the primary under the pointer
alone. The Figma design the operator approved needs exactly that, and asking the theme skill for
it got told it cannot be done.

## The decision

### 1. A component part is a named element of a family, in one closed manifest

A **component part** is a named, stable element of a component family: the button's root, label
and icon. Each family declares its parts exactly once, in the window's wrapper
(`data-scope`, `data-part`) and in the theme engine's **closed, versioned manifest**
(`purlis_core::theme::parts::MANIFEST`), which `purlis theme parts` prints as JSON and the
published schema is generated from. The manifest only grows: a family, part, variant or property
is added under a new version and never taken away. A theme can name only what the manifest has.

### 2. One key grammar, a closed property list, colours by token

A theme's `theme.json` gains a top-level `parts` object, additive to its format: a theme without
it is unchanged. Its keys are `<family>.<part>[.<variant>][.<state>].<property>`, the state at
rest implied. The states are hover, pressed, selected, focused, disabled and open (and invalid,
[amended 2026-10-26](0103/2026-10-26-a-refused-box-is-the-invalid-state.md)). The
properties are a closed list: fill, text, border colour, border width (0 allowed), corner radius
(one value or per corner), shadow (an elevation step), padding (block and inline), gap, font
size, font weight and height. There is no opacity and no raw CSS.

A colour names a semantic colour token, never a hex or a palette name, so palettes, appearances,
tints and the floors keep working for a part as they do for a token. A size is a number inside
its bound or a geometry token, which follows the person's density. A key that names something
the manifest does not have is said and left out, as an unknown token is; a value outside its type
or bound refuses the theme, with a sentence that names the bound.

### 3. The bounds keep a control a control

- **Corners** go from 0 to `full` on every part that draws a box, and are clamped as they are
  drawn to half the part's height, its variant's own where the theme sets one. `full` is the
  largest corner, a pill at any height. This lifts the `radius.*` caps for a part.
- **Padding, gap, font size and height** are bounded so a control stays at least 24px tall and
  its words, padding and border fit in it, in both densities. A geometry token is held to the
  same bound at the value the theme draws it at. A theme that breaks any is refused with the
  density named.

A panel, a section and an empty state are boxes, not controls: they are no target, and each
of their corners is clamped to half their own box ([amended 2026-10-22](0103/2026-10-22-a-box-is-no-target.md)).

### 4. A state never changes size

Inside a state only fill, text, border colour and shadow may be set. Padding, font size, font
weight, border width, height, gap and corners inside a state are refused, and the check says why:
a state never changes a part's size, so nothing moves when the pointer, a press, the keyboard or
a choice reaches it.

### 5. What a theme does not set is the family default

A property a theme leaves unset is the family default, which the window's own stylesheet draws
from the global tokens. There is no wildcard. **A theme that names no parts draws exactly as
before**, because the engine writes nothing for it.

### 6. The engine resolves the parts and writes one stylesheet

The Rust theme engine reads the parts, layers a child's over its parent's, holds the bounds and
the floors, and writes **one stylesheet** for the window, handed over the existing channel with
the resolved theme. Its rules are keyed on the wrapper's `data-scope` and `data-part`, its
`data-variant` and React Aria's state attributes, written without specificity so their order
alone says which wins: a state over rest, a variant over every variant. Every value is written
from its parsed form, never copied from the file (ADR 0041, property 4). The window puts it in
the `parts` cascade layer, above the controls and below forced colours. Semantic custom
properties keep their names, and parts are not Tailwind utilities.

The button family's rules also reached today's control class while the window was drawn by
one; the family now draws from its wrapper alone ([amended 2026-10-21](0103/2026-10-21-the-button-family-is-reached-by-its-attributes-alone.md)).
The field families join at version 3, the first with no variants
([amended 2026-10-22](0103/2026-10-22-the-field-families-and-a-family-with-no-variants.md)).

### 7. Floors hold per part and per state, and the consent floors hold for parts unchanged

In every variant and state a theme's parts change, the engine measures each part's words on its
fill at 4.5:1, an icon at 3:1, and a border that is the part's only edge at 3:1, on every surface
the family is drawn on. A disabled control is exempt, as WCAG has it. A part that breaks one
draws its family default there, and the theme check says so.

The surfaces a family is drawn on are its manifest entry's grounds, and they are every surface
the window draws the family on, found from the components by a test in the window that fails
until the manifest lists a new one (#2959). A wash is measured laid over the surface under it.

States that hold at once (hovered and pressed, focused and selected) are measured as the
stylesheet resolves them, each property from the state it writes last, and held like one state;
a part that breaks a floor in such a set draws its default in the last of its states, and the
finding names the set (#2958).

**ADR 0097 §6 holds for parts unchanged.** The consent floors are held whatever part of a family
the theme changes, as the theme is written and in every workspace tint, custom colours included,
and two looks are compared by the colours they draw, never by the tokens' names:

- an answer that grants keeps its accent edge: a border width of 0 on it is refused, a theme
  that turns every border off still draws it, and its edge contrasts 3:1 with its fill;
- Allow is distinguishable from Keep blocked by its edge, and is as plain as Keep blocked: in
  every state it is drawn on Keep blocked's fill;
- **Allow never takes the primary look**: parts never draw a consent variant nearer the primary
  than the family's defaults draw it in the same theme, measured on what a person reads as the
  look as it shows on the family's grounds;
- danger is distinguishable from primary: its words stay apart from the accent and from the
  primary's words, on the consent floors' scale.

A consent variant that breaks one draws its own default there; where that is not enough, so
does the variant it is held apart from. Allow's lock is the window's markup, and no property
reaches it.

## What was rejected

- **Raw CSS or a CSS subset for parts.** A theme's values stay grammars purlis parses (ADR 0041);
  a filtered CSS is the version of that rule that fails.
- **Hex colours in parts.** They would bypass the palette, the appearances, the tints and the
  floors, which all work on tokens.
- **A wildcard key** (`*.root.corner-radius`). It would reach families that do not exist yet, so
  what a theme draws would change when purlis adds one.
- **Size changes inside a state** (a heavier selected tab). They move neighbours, which the
  operator ruled out for every family (#2800).
- **Parts as Tailwind utilities, or a stylesheet per family.** One generated stylesheet, keyed on
  the wrapper's attributes, is one place to read and one place the engine writes.

## Consequences

- A theme can give purlis the shapes of the Figma design, part by part, within bounds that keep
  it readable and keep every control a target.
- A family joins the manifest with its parts and its defaults, under a new manifest version (the
  next release's, [amended 2026-10-25](0103/2026-10-25-one-manifest-version-per-release.md)), and
  its wrapper sets `data-scope`, `data-part` and `data-variant`.
- `purlis theme parts` and the schema are what the theme skill, the gallery and an editor read.
- The family default is the window's own stylesheet, and a test holds the engine's copy of each
  family's colours to it, so the floors measure what the window draws.

## Amendments

The amendments to this decision are files of their own in [`0103/`](0103/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. The latest:
[2026-10-24, the choice families join the manifest](0103/2026-10-24-the-choice-families-join-the-manifest.md);
[2026-10-25, one manifest version per release](0103/2026-10-25-one-manifest-version-per-release.md);
[2026-10-26, a refused box is the `invalid` state](0103/2026-10-26-a-refused-box-is-the-invalid-state.md).
