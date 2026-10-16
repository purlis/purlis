# Noted 2026-10-17: the core's theme engine holds it (purlis#2320)

The decision stands; where it is held moved. A theme file is now read, validated and resolved by
one theme engine in the Rust core (`purlis_core::theme`, spec #2313), and the window only applies
the resolved theme it is handed.

- **Geometry is refused by the engine.** A theme file that sets a core geometry name, in any
  spelling (`space.4`, `--radius-md`, a `geometry` block), is resolved without it, and the engine
  says why in its complaints. Its tests hold a theme file that tries every spelling;
  `app/src/theme/geometry.test.ts` still holds that `geometry.css` alone gives a core token a
  value, and that nothing the window writes from a built-in or a tint of one is geometry.
- **The vocabulary is the engine's.** The colour and motion token names and the geometry names
  a theme may not set are written once in `purlis_core::theme::vocabulary`. The window names the
  lists it writes, and `app/src/theme/core.test.ts` holds each to the engine's.
