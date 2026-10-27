# Amended 2026-10-27: the Runners view is Devices

**Recorded for #2052**, under spec #2041. The spec was ruled on 2026-10-14 and is the person's
approved word. It says: *"The window: the Runners view becomes Devices"*. Its story 54 reads:
*"As a person, I want the Devices view to show this Mac and every runner with its chats,
browsers, version, health and last contact, so that I know where everything is."* Story 55
reads: *"As a team admin, I want each pool's use per person (claims running and suspended, hours
this week, CPU and memory requested, disk held), and money only where a pool declares a price
per hour, so that cost is visible and never guessed."* Story 6 reads: *"As a person, I want
nothing about pools, claims or runners to appear until I add a pool, so that the first hour stays
simple."* This file adds no ruling. It records how §12 (and V50, which §12 settles) reads now
that the spec has spoken.

1. **§12's Runners view tab is the Devices view**, which shows *"this Mac and every runner with
   its chats, browsers, version, health and last contact"* (story 54), and each pool's use per
   person (story 55). §12 was never drawn under its old name.
2. **Nothing of it appears until a pool is added** (story 6).
3. **The view only reads.** It never wakes a suspended runner to ask it anything (the amendment
   of 2026-10-27 on suspended runners). §12's controls (start, stop, destroy, pin, expose, grant
   and the budget) and a runner's streams (its shell, log stream, ports and browser frames) are
   not drawn yet. They come with the tickets that build them (#3494, #3071). Until then, §12's
   rule stands for each: every control is a human act on `local-ui`.
4. **Money shows only where a pool declares a price per hour** (spec story 55). No pool declares
   one yet (#3491), so the view shows none. §12's cost and spend so far wait for the same.
