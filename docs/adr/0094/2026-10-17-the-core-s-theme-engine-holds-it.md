# Noted 2026-10-17: the core's theme engine holds it (purlis#2320)

This record's rule, superseded by [ADR 0097](../0097-a-theme-is-bounded-data-and-may-set-every-token.md),
is now held by the core's theme engine (`purlis_core::theme`, spec #2313) and no longer by the
window, until the theme format 2 lands. ADR 0097 §7's interim clause, that geometry stays core's
until the engine is built, keeps geometry refused meanwhile: a theme file is read, validated and
resolved by the engine, and the window only applies the resolved theme it is handed.

- **Geometry is refused by the engine.** A theme file that sets a core geometry name, in any
  spelling (`space.4`, `--radius-md`, a `geometry` block), is resolved without it, and the engine
  says why in its complaints. Its tests hold a theme file that tries every spelling;
  `app/src/theme/geometry.test.ts` still holds that `geometry.css` alone gives a core token a
  value, and that nothing the window writes from a built-in or a tint of one is geometry.
- **The vocabulary is the engine's.** The colour and motion token names and the geometry names
  a theme may not set are written once in `purlis_core::theme::vocabulary`. The window names the
  lists it writes, and `app/src/theme/core.test.ts` holds each to the engine's.
