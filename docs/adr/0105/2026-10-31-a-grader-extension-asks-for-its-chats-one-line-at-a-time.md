# Amended 2026-10-31: a Grader extension asks for its chats one line at a time, held to the plan

Ticket #3236 (SK-56), the follow-up §1 and ADR 0101 §5 left to it: the Grader **extension's**
question. It adds to §1, §3 and §4, and to ADR 0101 §5's sentence that a grade is one long
question with requests inside it.

**The question is protocol 4 of the executor** (`executor::PROTOCOL`), the first question of more
than one line. purlis writes the grade request (`studio` `grader`, `ask` `grade`, the request under
`request`) and keeps the program's stdin open. Each line the Grader answers is one of two:

- **A run-chat request**, `{"charter": 4, "run_chat": {"arm": …, "prompt": …}}`. purlis runs it as
  a grade's chat, through the host its own runner asks (§3: the same chat, piece, sandbox, profile
  and model), and writes back `{"charter": 4, "chat_ran": {…}}`, the run-chat result as §1 spells
  it.
- **Its grade report**, `{"charter": 4, "report": {"triggers": …, "arms": …, "delta": …}}`, which
  ends the question.

An `error`, or any other line, ends the grade. A manifest that declares `grader` is version 4 at
least: a Grader's question is protocol 4's, and none was ever asked in 3.

**Each answer has the executor's five seconds from the line before it**; while purlis runs a
chat the Grader has no deadline, as each chat is held to its own time (§3). The question stops
when the person stops the grade: its program is stopped at once, whether a chat runs or the
Grader is between lines.

**What a Grader asks for is held to the plan and to the limits the person accepted**
(`grade::Held`, which purlis's own runner asks through too, so both are counted by one copy):

- **The plan**: each chat is of one of the grade's arms, on the query of one of its trigger tests
  as the plan showed them, and each test runs at most `runs` times in each arm. A chat past that
  is never run, and ends the grade. So a Grader runs no more chats than the plan showed, and no
  prompt the plan did not show.
- **The limits**: each chat is told the tokens left, and the grade ends at its token cap, at its
  dollar limit, after two chats that did not start, or after a chat that did not say what it
  spent, exactly as a grade of the built-in runner does (§4 and its amendments). Once it has
  ended, the next chat the Grader asks for is not run: it is written back as a result that says
  why (`over-cap`, `over-limit`, or `not-started` with the sentence), so the Grader can answer its
  report. A Grader that asks again has asked past its limits, and that ends the grade.

**Every message is read against a closed shape, failing closed.** A line is at most 512 KiB. A
run-chat request holds `arm` and `prompt` alone. A Grader's report holds its scores alone, each
part of it closed too: what a grade spent, how it ended and who graded it are purlis's to say, so
a report that carries them is refused. The report's tests are the grade's own, in its arms; each
chat it names is one this grade ran, once; each arm is scored once; and it holds together as a
row of the eval record is read back (§5), or it is refused, since a report the record would skip
is none.

**Every way it ends is a report**, recorded as any grade is (§5):

- **The Grader's**: its scores, `grader` the extension's id, with purlis's own count of what the
  chats spent and the limits, ended as purlis saw the grade end (whole where it saw no other end).
- **A Grader that failed** (out of shape, past the plan or the limits, too late, none, or refused
  before it was asked): what ran, scored as purlis's runner scores it, ended `grader-failed`, with
  `failed`, a sentence that names the Grader and says why. The chats that ran are counted, as
  they spent the person's tokens.
- **A grade the person stopped**: what ran, scored, `cancelled`.

**A Grader never reaches a harness or a model itself.** It is confined (ADR 0101 §2 and §3): it
reads its own folder alone, so no harness's program, and its proxy carries no model's host. The
studio probe's Grader tries both from inside its grade, and its test asserts each is refused,
beside a grade whose every chat was one purlis ran.

## Rejected

- **Letting a Grader name any prompt.** The person accepted the chats the plan showed, each trigger
  test as the chat will read it. A judge's chat, with outcome evals (#3235), will add a prompt
  purlis names, not one the Grader writes.
- **Trusting a Grader's figures for the spend or the end.** Those are what the cap and the dollar
  limit hold; a Grader could say anything of them.
- **Ending the grade at the first chat asked for after its end.** The Grader cannot know that a
  chat which ended its turn took the grade to its cap; one answer that says so lets it report.
- **A deadline for the whole question.** A grade runs as many chats as its plan has, each up to
  five minutes; the question is bounded by the plan, the limits, each answer's deadline and Stop.
