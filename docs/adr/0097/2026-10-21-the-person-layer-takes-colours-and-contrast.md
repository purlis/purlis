# Amended 2026-10-21: the person's layer takes colours, a minimum contrast and reduced motion (purlis#2925)

The operator's ruling #2814 (items 7 and 8), which spec #2897 carries. §6 says density, text
size, the font override and reduced motion are the person's settings, and that every theme
honours them. The ruling adds:

- **Reduce motion and minimum contrast are settings in purlis, on top of the system's.**
  Purlis's own reduce motion and minimum contrast are layered on the operating system's
  settings.
- **The person's layer takes colours as well as density and fonts.** A person can override
  single colours on top of any theme without forking it.
- **The floors and the consent rules still hold.** They hold over the person's colours and the
  person's contrast as they hold over a theme's own values.

How purlis builds these (the raised floors, how a colour is refused, where they are kept and
drawn) is in `docs/design-system.md` and in the decisions of purlis#2925.
