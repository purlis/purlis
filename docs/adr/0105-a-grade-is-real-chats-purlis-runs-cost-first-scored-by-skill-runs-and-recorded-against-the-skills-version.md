# A grade is real chats purlis runs, cost first, scored by skill runs and recorded against the skill's version

**Accepted 2026-10-25**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20 (decisions 8, 21, 22, 23, 27, 43 and 45), and the
delegated decision of 2026-10-20 on #2779 (the fixture files and the built-in runner's method);
ticket #2779 (SK-16). It is the Grader capability's own record, which
[ADR 0101](0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md)
§5 left to #2779: the four messages' fields and wire spelling, the built-in runner, the eval
record, and how a grade's chats are run. It amends
[ADR 0061](0061-a-curation-action-is-a-chat-with-its-prompt-typed-and-never-sent.md) with a
second reason a chat starts on a sent message, in [`0061/`](0061/).

**Built by #2779** (trigger tests, the record, the plan and the window's Grade) and its follow-ups
(outcome evals, the Grader extension's protocol, the previous version as an arm).

## Where purlis was

ADR 0101 named a Grader, the built-in runner as its first user, and four messages between purlis
and a grading framework, and said a grade's chats are purlis's to run: on a scratch workspace,
under the chat sandbox, on the harness and model the person chose, with the cost shown first. It
did not say how purlis runs a chat no person types into, how a grade is scored, where it is kept,
or what a person sees before it runs. purlis had no chat that runs on a prompt from a file: every
prompt it prepares is typed and left for the person to send, but for one button's fixed message.

## The decision

### 1. The four messages

The Grader's interface is four messages, spelled in `purlis_skills::grade` and used unchanged by
the built-in runner, so an extension and purlis's own runner speak the same shapes. Each is JSON,
with snake_case keys and kebab-case words.

| Message | Fields |
|---|---|
| **Grade request** (`GradeRequest`) | `skill`; `triggers` (`[{query, should_trigger}]`); `outcomes` (`[{id, prompt, expected_output, files, expectations}]`); `arms` (`with`, `without`, `previous`); `runs` |
| **Run-chat request** (`RunChat`) | `arm`; `prompt` |
| **Run-chat result** (`ChatRan`) | `chat` (the chat's id, which opens it); `started` (each skill a run started for in it, by the name its harness started it by); `tokens`; `cost_usd`; `ms`; `end`: `turn`, `timed-out`, `not-started` with `why`, or `cancelled` |
| **Grade report** (`GradeReport`) | `grader`; `triggers` (each test in each arm: `runs`, `triggered`, `rate`, `pass`, `chats`); `arms` (each arm's `pass_rate`, `tokens` and `seconds` as `{mean, stddev, min, max}`, `chats`, `not_run`); `delta`; `ended`: `whole`, `cancelled` or `chats-did-not-start`; `not_started` |

A run-chat request names no harness, model, folder or sandbox: those are the person's and
purlis's, chosen before the grade starts. A judge's chat, when outcome evals arrive, is a
run-chat request of its own, with what it reads named by purlis.

### 2. A trigger test is scored by the skill runs its chat started

A chat **triggered** the skill when a skill run of it started in the chat (ADR 0101 §5, #2766):
whichever route its harness took, the harness's own Skill tool, a read of a briefed `SKILL.md`, or
`skill_load`. A plugin's namespace is no part of the name (`purlis-project:deploy` is `deploy`).
A test passes in an arm when the share of its runs that triggered is at least one half for a
query that should trigger it, and below one half for one that should not, as skill-creator scores
it. Each arm's pass rate is the share of tests that passed in each run, as a mean and a sample
standard deviation over the runs, and the **difference** is the arm with the skill's mean less the
baseline's: the previous version where the grade has one, else no skill.

**The built-in runner follows skill-creator's method**: every test in every arm, 3 runs each. It
asks for its chats run by run, every test and arm in each, so a grade the person stops has scored
the arms alike. Two chats in a row that do not start end the grade, with the last one's sentence:
every chat after them would fail the same way.

### 3. A grade's chat is a chat purlis starts on the fixture's query, and draws nowhere

For each run-chat request purlis lays out a **throwaway piece** for the arm, as Try it does (#2772):
the skill as the project holds it, **without its `evals/` folder**, so the chat a fixture runs in
is never handed the fixtures it is graded by; or, for the arm without it, no project skill at all.
Either way the chat has the built-in skills, and the harness's own plugins and skills are off, as a
Try it's chat has them. It starts a chat there on the profile the person picked (or the project's
default), under the project's sandbox, with the **query as its first message, sent**: a grade
measures what the chat does with the query, so it is sent as it is written (ADR 0061, amended
2026-10-25). Its skill runs say a grade started them (`started_by: grade`), so a skill's run
totals tell its uses from its grades.

**purlis watches the chat and ends it**: once the skill under test starts (a trigger test learns
nothing more from the rest of the turn, and every further token would be spent for nothing), once
its turn ends, or after five minutes. One chat runs at a time.

**A grade's chat is drawn nowhere**: it has no tab, is listed nowhere, and a relaunch never starts
it again (the record's `graded`). The window shows the grade instead: how many of its chats have
run, Stop, and how it ended.

### 4. The cost comes first, and the person's account pays

Nothing runs until the person presses Grade on the **plan**: the chats the grade runs, each one
turn, each trigger test as the chat will read it, with every character no reader would see marked,
the tests left out because no chat can be started on them, the outcome evals that wait, and what
the copy's last grade spent per chat, worked out for this one. **No figure is shown that was never
measured**: a skill never graded on this project says that what a grade spends is not known yet.
purlis keeps no price list: the tokens and dollars are what the harness itself reported for the
last grade's chats.

The chats run on the profile the person picks and **are paid for by that profile's account, as any
chat is**; the plan says so. A grade starts on the person's press alone, or, once the opt-in lands,
after an edit the person made; never on a schedule (decision 22).

### 5. The eval record

A grade is one row of `personas/_grades/<YYYY-MM>.<device>.jsonl`, append-only and committed
beside the skill log (`docs/plane-format.md`). It holds the copy (its skill and level), **what its
folder held by content**, the tree git holds of it as a skill run's start records it (or that it
held changes not committed), the commit that tree is, the harness and profile, when, and the
report and what the chats spent. So a grade is tied to the skill's version by content, wherever it
ran: a teammate who pulls the record sees it on the same version, and the skill's history shows
each version's newest grade. Results never go in the skill's folder (decision 21).

The record is read as a teammate's purlis may have written it: each file at most 16 MiB through no
link, each line at most 1 MiB, and a row that says more than purlis writes (a commit that is no
hex id, a harness or profile that is no plain name, a stamp that is no time) is skipped.

## What was rejected

- **A first-tool-call probe** (skill-creator's `run_eval.py`): it works only on Claude Code and
  tests only the description. Skill runs work on every harness and are what the studio already
  records.
- **Harness print modes** (`claude -p`, `codex exec`, `opencode run`) for a grade's chats: a chat
  run outside the app's chats has no event log writer, so its skill runs are not recorded as every
  other chat's are, and its sandbox and arming would be a second copy of the window's.
- **Running every turn to its end.** A trigger test's answer is known once the skill starts.
- **An estimate from a price list.** Prices change per model and plan, and a figure purlis made up
  would be the one place a grade says something unmeasured.
- **Keeping results in the skill's folder.** They would travel with the skill and be edited with
  it; a grade is evidence about a version, not part of it.
- **A grade's chats as tabs.** A grade runs many short chats; tabs opening and closing would take
  the window from the person.

## Consequences

- ADR 0061 has a second exception to "typed and never sent", for a grade's chats alone, pinned by
  the test that pins the first.
- A grade spends the person's tokens. The plan says how many chats, and what the last grade spent.
- A grade's chat that asks the person something waits for nobody and is ended at its time; its
  test counts what it did until then.
- The skill log holds grade runs, marked as theirs.
- Outcome evals, the previous version as an arm, the Grader extension's protocol over the
  executor, the opt-in after each edit, opening a grade's chat, a model picked per grade, the
  terminal running a grade, and routers graded by trigger tests follow as tickets of M65.

## Amendments

The amendments to this decision are files of their own in `0105/`, one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record, so two pull requests that amend it do not touch the same lines
(#1982).
