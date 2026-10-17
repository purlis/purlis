# Noted 2026-10-17: a seventh type step, for a tight label (purlis#2319)

The core type scale this record set at six steps has a seventh below them, `--font-size-2xs`,
10px at the 14px root (`calc(10rem / 14)`), by the operator's decision of 2026-10-17. It is for
a tight secondary label only: the Chats view's and the Inbox's scope switch
(`.ui-levels-small`), which at 11px ran past the Chats view. The words' floor stays 11px
(`--font-size-xs`, `docs/design-system.md` "Text roles", rule 3), and a migration's snap never
lands under it.

The step is core geometry like the rest: `app/src/theme/geometry.css` gives it its value,
`app/src/theme/geometry.ts` names it, and the core's theme engine refuses it in a theme file
(`purlis_core::theme::vocabulary::GEOMETRY`).
