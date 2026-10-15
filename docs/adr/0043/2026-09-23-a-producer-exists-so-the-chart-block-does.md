# Amendment, 2026-09-23: a producer exists, so the chart block does

**This amendment is not a sign-off, and the DRAFT line at the top of this record still stands.**
It records what changed when ADR 0041's stage 2 was built (see that record's amendment of the
same date, and purlis#212, unmerged when this was written).

**Panels may now be fed by a producer — through a view, and not on the panel's own path.** The
record's rule for *where a panel's data comes from* is unchanged: `workspace_panels` answers in
one call inside the 100 ms a workspace focus has, and a contributed panel is declared. What an
extension answers *live* is a **view**: a surface the operator opens, which purlis fills by
asking the extension's program once, through the executor's gate, when it is opened. That is
the *"second surface opened from a row"* the statistics section above named, with one change of
place and one of name:

- **It is opened from a panel's heading, or from inside a card — not declared as a `Detail`.**
  A view says what it is *about* (`panel::Subject`, a closed set of one: `personas`), and
  purlis decides where things about that subject are offered. purlis's own personas panel is
  about personas, so a view about personas is a button on its heading and a section in each
  persona's card. The extension chose a subject and nothing else — property 3, *purlis chooses
  the consumer*, carried to the first contribution that runs.
- **A declared panel cannot claim a subject.** `about` is set only by purlis's own producer;
  a manifest that tried would be choosing which of purlis's surfaces a stranger's view appears
  on.

**The chart block exists, and only in an answer.** `Block::Chart { title, shape, unit, points }`
— a shape out of `bars | columns`, each point a label, a whole count below 2^32 and a short
note. The objection this record raised stands for a manifest and does not apply to an answer:
*a declared chart is a chart of numbers the extension wrote down at install time*, and a
program asked when the operator opens a view answers with numbers read at that moment. So
`panel::declared` still has no key a chart could arrive in and refuses one by name, and
`panel::answered` accepts one. Everything the vocabulary refused before, it refuses in an answer
too: an unknown key, a control character, a colour, a `runs` on a row, an `offer` on an empty
state. The window draws a chart from theme tokens, scales it itself, writes every label as a
text node, draws it as a list to a screen reader, and animates nothing.

**The first producer, and why its three charts.** `crates/persona-statistics` answers with the
number of memories per persona (bars — *which persona carries this plane's knowledge*), memories
written in each of the last twelve weeks (columns — *is this plane still learning*), and days
since each persona last learned something, quietest first (bars — *which persona has gone
quiet*, which is the one an operator has a next step for). Opened from one persona's card it
answers about that persona first.

**#173's popover, which this record left open, is answered.** The persona card is now a
**non-modal Radix `Dialog` portalled into the centre region** — over the terminals, and nowhere
else. Every reason #173 gave for a popover is kept: not modal, so nothing outside it is
`aria-hidden` and the needs-you queue stays in reach (a jsdom test that goes red with `modal`
removed holds that); not the whole window, which is the alerts drawer's; dismissed by Escape or
a click outside; and still the one piece of window state, so `persona.show:<name>` opens it from
the palette. What it gave up is being anchored to the row. Every other row's card is still the
popover, because six short rows are what a popover is for. **This is a decision made without
the operator**, and it is the one in this amendment most worth his attack.

**The asymmetries are unchanged**, and the reason is now sharper. A view runs code, and still
cannot put a purlis verb on a row: what an extension puts in the window is data, whether its
manifest declared it or its program answered it. The grant that would lift that — *this
extension may offer this catalogue row* — needs no new mechanism now; it needs a plugin that
wants it, and none has asked.

**The consequence this record predicted is paid off in part.** *"A contributed panel is inert
and will disappoint"* — a contributed panel still is. A contributed *view* is not: a stranger
can now ship a chart of something current, and the persona statistics view is the proof that
the path works end to end, installed, approved, fingerprinted and asked.
