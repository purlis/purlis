# Amended for #4634 (2026-11-02): a task question is counted from its steps, and never read

Spec #4551 and ADR 0107 ask telemetry to count how many task questions reach the person, why,
and how long they wait. #4557 writes each step of a question's life to the event log as a
`question.*` event (`docs/plane-format.md`, the event log's row), which ADR 0075 makes audit
metadata. This amendment records what the exporter reads of those events, and that a
question's words are never among it (ADR 0075 §1).

- **Each step is a log record of its own,** derived from the event log as §1 derives every
  other (V25d), with §6's per-record attributes. The record's event name is the event's
  `kind`. A count is how many records a backend counts, grouped by an attribute. purlis sums
  nothing on the device.
- **The record carries these attributes, and only these.** Each comes from a field of the
  event's body. Every value is a whole number, a flag or a word from a closed set:

  | Attribute | Value | From |
  |---|---|---|
  | `charter.question.number` | the question's number within its task | `number` |
  | `charter.question.noted` | whether the asking chat added a note as it passed it on | `noted` |
  | `charter.question.reached.why` | `passed_on`, `no_reply` or `asker_gone` | `why` |
  | `charter.question.by` | `asking_chat`, `person` or `nobody` | `by` |
  | `charter.question.waited_secs` | seconds since it was asked, or for an answer from the person, how long it waited on them | `waited_secs` |
  | `charter.question.failed.what` | `not_read_back` or `not_redirected` | `what` |

- **What a maintainer reads off them.** The questions that reach the person are the
  `question.reached` records, grouped by `charter.question.reached.why`. How long the person
  keeps them waiting is `charter.question.waited_secs` on the `question.answered` records whose
  `charter.question.by` is `person`. Open questions, and questions the asking chat answers, come
  from `question.asked` and the rest of the steps.
- **No words, by construction.** A question's text, its answer and a note have no field in a
  step (`purlis_core::eventlog::questions::Step`). A note is reduced to the `noted` flag. Two
  tests hold this. `neither_the_audit_nor_telemetry_ever_holds_a_question_s_words` (in
  `dispatchtalk`) walks a question through its life with marked words and finds none of them
  in what is written.
  `telemetry_can_read_of_a_question_only_the_fields_its_record_names_none_of_them_words` (in
  `eventlog::questions`) fails when a step writes a field this table does not name, or a value
  that is not a number, a flag or a closed word. A new field therefore changes this record first.
- **The names take §6's prefix, `charter.*`,** as the 2026-10-23 amendment's do, and move with
  it if §6 is renamed.

## Decided in implementation (#4634)

- **D-4634a: the exporter is not built here,** as D-SK20a decided for skill runs. purlis has no
  OTel receiver, store or exporter yet, so nothing exports these counts today and a maintainer
  cannot yet read them anywhere. The events are already where the exporter will read them, so
  this change records the mapping and adds a test that holds the event bodies to it. The OTLP
  export lands with OB-18. Rejected: a mapping function or an on-device tally written now, with
  no caller but its tests.
