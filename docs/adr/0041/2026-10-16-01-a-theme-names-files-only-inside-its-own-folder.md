# Amended 2026-10-16: a theme names files only inside its own folder (ADR 0097)

[ADR 0097](../0097-a-theme-is-bounded-data-and-may-set-every-token.md) lets a theme set every
token, its fonts and its icon set included. The four properties of a declarative extension
stand, as ADR 0097 §3 restates them. The third crossing, **a reference to a file**, is held by the
theme folder:

- a font is a woff2 file inside the theme folder, under a size cap, and an icon set is a file
  inside it in today's path grammar;
- each is named relative to the folder and read only from inside it. A path that leaves the
  folder, a link or a URL is not a value the grammar has, and it refuses the theme;
- purlis reads the file and serves it to the window itself. A theme never names a destination.

A palette name a token refers to is local to the theme that declares it, and names no file.

**Built by #2322** (theme folders) and #2326 (a theme's fonts). Until then, a theme names no file,
as today.

The other two crossings, a value that reaches a CSS context purlis did not choose and any way of
saying *where*, are unchanged. The consent surfaces and the state colours are amended separately
(the amendment of 2026-10-17 on consent surfaces).
