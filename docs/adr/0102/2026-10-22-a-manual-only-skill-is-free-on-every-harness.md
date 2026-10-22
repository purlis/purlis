# A manual-only skill is free on every harness

**2026-10-22**, ticket #2771 (SK-8), amending
[the skill budget and Use skill… as built](2026-10-21-the-skill-budget-and-use-skill-as-built.md),
under the operator's spec #2762 (decision 19).

The 2026-10-21 record counts a manual-only skill on Codex and opencode, "still listed on this
chat", until SK-8 hands each harness the translation. SK-8 does
([ADR 0063 as amended 2026-10-22](../0063/2026-10-22-a-skill-is-handed-only-what-narrows-a-chat-and-each-harness-its-translation.md)):
Codex's briefing names a manual-only skill and its file alone, and opencode is handed it without
its description, which keeps it out of its listing.

So **a manual-only skill costs nothing wherever its harness keeps it out of the listing**, by its
own key or purlis's translation. `Budget::manual_free` is no longer each adapter's word: it
follows from what the harness reads (`Harness::skill_budget`, `skills::manual_kept_out`), so it
cannot disagree with what the chat is handed. On the three harnesses purlis ships, it is free on
all three. A harness with no way to keep one out still counts it, and the budget still says
"still listed on this chat" there.
