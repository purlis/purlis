# Amended 2026-10-28: a pool's runner suspends after its pool's own time

**Built by #3471** (the person's ruling of 2026-10-28). Item 4 of the amendment of 2026-10-27 is replaced: a day is the default, and a pool sets its own time before its runner suspends itself as `suspendAfter` in `runners.json` (`docs/plane-format.md`), a duration from a minute to 30 days, given to its runner as it is made (`purlis serve --suspends-unlinked=<duration>`). A cluster pool's equivalent on the operator's `Pool` is #3703.
