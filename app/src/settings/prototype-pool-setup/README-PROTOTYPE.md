# PROTOTYPE: Pool setup (spec #4469)

Throwaway. It lives on branch `prototype/pool-setup` and never merges to main. Rulings: Q24–Q51 on
#4467 and Q1–Q23 on #4464 (Q6 amended by Q40).

## The question

Can someone who has never heard of a runner follow every step and state of Pool setup alone? Q29
fixes the layout, so this draws one design in full, not several variants.

## Open it

- The published page is the easy way. It is the same build as `PROTOTYPE-artifact.html` here.
- Or open the story: `cd app && npm run storybook`, then **PROTOTYPE › Pool setup › Every state**.
- Rebuild the page: `cd app && npx vite build --config src/settings/prototype-pool-setup/vite.standalone.config.ts`.

The bar at the bottom is the prototype's, not the design: `[` and `]` go through the states, `\`
lists them, and it switches light, dark and theme. Where a screen waits on the outside world, a
**Pretend** button plays that event. The real buttons walk the path. All data is stand-in: nothing
calls Docker, a cluster or a forge.

## Files

`data.ts` (steps, states, scenarios), `screens.tsx` (every step), `kit.tsx` (the shape of a step),
`PrototypePoolSetup.tsx` (the tab, the step list, the bar), `prototype.css`. Components come from
`app/src/ui`. The step list, the check lines and the bar are hand-drawn for now: the M66 family for
them is a question for the Figma round.

## Verdict

The person accepted the prototype on 2026-11-01. The layout, steps and states stand as drawn. Next
under #4469's delivery order: Figma designs on the M66 UI system.
