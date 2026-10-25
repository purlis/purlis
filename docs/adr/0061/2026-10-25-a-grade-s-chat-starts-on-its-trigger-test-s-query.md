# Amended 2026-10-25: a grade's chat starts on its trigger test's query, sent

Ticket #2779 (SK-16), building what the operator agreed on 2026-10-20 (spec #2762, decisions 22
and 43: a grade runs real chats; [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md) §5), recorded in
[ADR 0105](../0105-a-grade-is-real-chats-purlis-runs-cost-first-scored-by-skill-runs-and-recorded-against-the-skills-version.md) §3.
**This amendment asks for the operator's confirmation**: it is the first send whose text purlis did
not write.

A **grade's chat** is the second chat this record's machinery starts on a sent message
(`curation::start_graded`, `SentFor::Grade`). Its first message is one trigger test's query,
from the skill's own `evals/triggers.json`, sent as it is written: a grade measures what a chat
does with the query, so purlis can neither rewrite it nor wait for a person to send it a hundred
times.

- **Only the person's press starts a grade**, on the plan that showed every query first, each
  character no reader would see marked, and a query no chat can be started on (one a harness would
  read as its own flag) left out and named. The commands are the window's alone; no chat, tool,
  hook or `purlis` word reaches them.
- **The chat starts in a throwaway piece**, under the project's sandbox, with the built-in skills
  and the skill under test alone (or none), on the profile the person picked. It is drawn nowhere,
  is ended by purlis, and is never started again at a relaunch.
- **Nothing is typed**, so nothing is gated by the board, as for Fix.

Every other prompt this record prepares is still typed and never sent. A third reason is a new
amendment here, and a new `SentFor`; the test that pins `start_sent`'s one call site pins
`start_graded`'s too.
