# Amended 2026-10-26: a grade's limits are one decision, and hold what its tokens do not count

Ticket #2779 (SK-16), from the second security review of #3229. It makes §3 and §4 exact where the
review found a gap between what they say and what was built.

**The start says it is a grade's chat, and nothing else does.** Whether a chat starts with
purlis's MCP server alone, answering only the tools that read, is decided by the chat's own record
(`graded`), handed to the code that arms it. A start that says it is a grade's in a folder no
grade laid out, or one in a grade's folder that does not say so, is refused, as is a grade's chat
on any program but Claude Code, on a runner, or where purlis cannot arm it. A marker file is no
longer the word that decides it.

**What a grade cannot count is held too** (§4):

- **Any chat that started and did not say what it spent stops a capped grade**, however it ended:
  at the end of its turn, past its time, at an ask, at the cap, or on Stop. Each of those spent
  tokens, and the cap cannot be held after one whose spend is unknown.
- **A grade's chat starts no sub-agent.** Claude Code's tools that start one (`Task`, and `Agent`
  as newer versions name it) are denied it, as a sub-agent's spend is not in the tokens its chat
  reports.
- **A cost bound besides the cap**: the cap's tokens at the dearest price any model a grade can run
  on charges for one token (75 dollars a million, Claude Opus 4's output). Where the program says
  what a chat cost, the grade stops once its chats cost that, and the chat running then is ended,
  as at the cap. It never stops a grade before the cap whose tokens were all reported. It is one
  bound, not a price list: no estimate is made from it, and the plan still shows only what was
  measured. The plan states it beside the cap, and the report and the record keep it and what was
  spent (`cost_cap_usd`, `spent_usd`).

**A grade's chat keeps its permission prompts whatever the person's own settings say** (§3): its
session's settings set Claude Code's default permission mode, which outranks the person's, and a
grade is refused where the person's own Claude Code settings start every chat with the prompts off,
or cannot be read, as what they say cannot then be known.

**What runs, what was shown and what is recorded are one read** (§3, §5): the plan reads the
skill's copy once, its consent is a digest of that read, the grade lays out every arm from it, and
the record's tree and commit are what git said the folder held both before and after its files
were read; a folder that changed in between is recorded as holding changes not committed.

**A query's first character is the first one a program would act on** (§3): characters no reader
sees (a byte-order mark, a zero-width space) are passed over before it is checked against `-`,
`/`, `!`, `#` and `@`.
