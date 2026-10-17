# Noted 2026-10-17: geometry and fonts are read, and how purlis serves a font (purlis#2326)

§7's interim clause ends for geometry and fonts: the core's theme engine reads both from a theme
folder (`purlis_core::theme::geometry`, `purlis_core::theme::fonts`), and the window writes what
the engine resolved.

- **Geometry is a number in its documented range.** Each geometry token has a range in the
  engine (`geometry::STEPS`), published in the theme schema and in `docs/design-system.md`. A
  value outside it, a fraction where a whole number is due, or anything that is not a number
  refuses the theme, and the complaint names the range. The engine writes every custom property
  from the number; core's values are Purlis Classic's, and a test holds what the engine writes
  for Classic equal to `app/src/theme/geometry.css`.
- **Density and the font override are the person's** (§6). The engine resolves a comfortable and
  a compact geometry for every theme; core defines Purlis Classic's compact, and a theme tunes
  its own in a `compact` layer held to the same ranges. The person's layer (`theme::Person`) is
  read by the engine from the layout file and drawn over the theme in force: the person's
  density picks the geometry, and a family the person names, held to a closed grammar and
  written by the engine, is drawn in place of the theme's.
- **How purlis serves a font (§2, §3).** A face is a file `fonts/<name>.woff2` inside the theme's
  folder: one plain name, reached with no link on the way, under a 2 MiB cap, with the woff2
  signature. The engine reads it with the theme and hands the window its sha-256, never a path
  or an address. The window asks for the bytes by that digest over the app's own IPC
  (`theme_font`, the window's alone), and the engine answers only a digest of a face of a theme
  it read and accepted (a refused theme registers none), reading that file again under the same containment and cap and
  answering only while the bytes still have that digest. The window makes the face with
  `FontFace` from the bytes. So no URL scheme or file address exists for a page to reach, the
  window's content security policy is unchanged, and the stylesheet names a face only by a name
  the engine made from its digests; the theme's family label never reaches a declaration.
- **Cold start.** The first frame does not wait for a face: it is drawn in each family list's
  fallback, and the faces are asked for after it. The Linux cold-start gate launches every
  measured profile with a theme of the person's that brings fonts.
