# Amended 2026-10-25: the Grader's four messages are spelled in ADR 0105

Ticket #2779 (SK-16). §5 left the fields and wire spelling of a grade's four messages to #2779.
[ADR 0105](../0105-a-grade-is-real-chats-purlis-runs-cost-first-scored-by-skill-runs-and-recorded-against-the-skills-version.md)
records them, with the built-in runner that speaks them, how purlis runs a grade's chats, the
cost shown first and the eval record.

Two of §5's words are made exact there:

- **The arms.** The built-in runner compares the skill with no skill now. The previous version
  as an arm, which §5 names, follows: its chats need the skill laid out as its earlier commit had
  it.
- **A trigger is a skill run of the skill under test**, by its name without a plugin's namespace.
  purlis ends a trigger test's chat as soon as that run starts, since the test learns nothing
  more from the rest of the turn.

The Grader **extension's** question, a grade request whose answer comes after requests of its own
(protocol 4 of the executor), follows as its own ticket. Until then the built-in runner, in core
code behind the same messages, is the only Grader.
