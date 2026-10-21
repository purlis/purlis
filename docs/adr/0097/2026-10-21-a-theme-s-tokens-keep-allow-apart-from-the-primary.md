# Amended 2026-10-21: a theme's tokens keep Allow apart from the primary (purlis#2948)

§6 lists the consent floors. It gains one: **Allow is distinguishable from the primary**, held
on a theme's tokens, not only on its component parts.

- **What is measured.** The button family draws an answer that grants (an Allow, an Accept) and
  the suggested answer from the theme's tokens. The two looks are compared as the component parts
  check compares looks (ADR 0103 §7): by the colours they draw, never by the tokens' names. The
  fills are compared as each shows on every surface the family is drawn on, with the nearest
  surface counting, and the edges count where they stand out. This holds at rest and in each
  state the two are drawn in.
- **Where.** In every appearance and every combination of option values, as the theme is written
  and in each of its own workspace tints.
- **What a theme that breaks it draws.** Purlis's own fill for the primary first, so Allow stays
  as plain as Keep blocked. Allow's own fill is drawn as purlis's only when that is not enough. A
  theme that clears it as written and would fall under it only in a tinted workspace keeps its
  values, with those fills drawn untinted there. The consent floors' Notice names the floor, and
  the rest of the theme draws as written.
- **The rule** is `Floor::AllowNotPrimary` in `purlis_core::theme::floors`, measured by
  `theme::parts`. `theme/allow_primary_tests.rs` holds each case.

The built-in themes clear it in every appearance and tint, and are drawn unchanged.
