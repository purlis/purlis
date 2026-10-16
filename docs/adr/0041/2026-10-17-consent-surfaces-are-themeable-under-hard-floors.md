# Amended 2026-10-17: consent surfaces are themeable under hard floors (ADR 0097)

The operator ruled on 2026-10-17, recorded in
[ADR 0097](../0097-a-theme-is-bounded-data-and-may-set-every-token.md) §1 and §6. It amends this
record's rule that *the consent surfaces and the state colours are purlis's, not the theme's*:

- **The consent surfaces and the state colours are in the published theme vocabulary**, and any
  theme may set them. The consent surfaces are the asks, Allow and Keep blocked, and needs-you.
- **The engine's hard floors hold for them.** Beside the general floors, an ask contrasts with its
  surface, Allow is distinguishable from Keep blocked, and danger is distinguishable from
  primary. These are the consent floors. A theme draft that breaks any hard floor is not drawn,
  and the theme check reports any theme's floor failures.
- **The consent floors hold for every theme**: when a theme's consent-surface or state-mark colours
  break a consent floor, the engine draws purlis's own values for those tokens, with a Notice
  naming the floor; the rest of the theme draws as written.
- **A consent surface's wording, icons and placement stay purlis's** and are not theme data, so
  a theme cannot draw a prompt. The distinction this record draws between safety from code
  execution and safety from deception stands; the floors are how purlis holds the second for
  colour.

**Built by the theme engine tickets of spec #2313** (#2320, #2332). Until then, the window keeps
today's rule: the consent surfaces and the state colours are purlis's.
