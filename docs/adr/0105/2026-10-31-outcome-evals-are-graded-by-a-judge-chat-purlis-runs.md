# Amended 2026-10-31: outcome evals are graded by a judge chat purlis runs

Ticket #3235 (SK-55), from spec #2762 decision 8 ("outcome evals, checks or a model judge on
fixtures") and the delegated decision of 2026-10-20 on #2779 (skill-creator's method). It adds to
§1, §2, §3 and §4, and builds what §1 left for later: "A judge's chat, when outcome evals arrive,
is a run-chat request of its own, with what it reads named by purlis."

**Outcome evals are off until the person ticks them** (the operator's ruling of 2026-10-31). The
plan lists each eval it can run with the chats it would add, and only those ticked reach its
chats and its consent, which covers exactly them. A skill gaining an evals file spends no more,
and its plan's consent does not change, until the person ticks one. An opt-in to grading after each
edit keeps the ticked ids and their digest.

**A ticked outcome eval runs in each arm, three times, and a judge chat grades each of its chats.**
An eval with no expectation has nothing for a judge to check, so it runs no chat and the plan
names it as left out. The run-chat request says what its chat is for (`kind`): a trigger test's,
as before; an outcome eval's, with its `files`; or a judge's, naming the chat it judges (`of`) and
the task it grades (`task`). The run-chat result says what the chat's turn ended saying
(`answer`), where its harness handed it back.

**An outcome eval's chat may write and edit inside its own throwaway piece** (the operator's rulings
of 2026-10-31 and 2026-11-01, adding to §3), still under the sandbox. Outcome chats' writes are held
to their piece by a guard, and a write goes ahead only on its answer. A run whose guard did not
answer fails, with the reason in the record, and so does one whose chat asks for anything else.
No other chat of a grade gains this.

**An outcome eval's chat runs its whole turn** (§3), in a throwaway piece laid out as the arm's
is, with the eval's input files staged into it at the paths the eval names. Each file is read as
text, at most 1 MiB and through no link, in the same read as the skill's copy, so the consent
covers it; an eval's files must be its own inputs under `evals/`: one that names a file of the skill (it
would hand the arm without it the skill), a fixture file in any case, a chat's instruction file, or
a file that does not read is left out and named, as is one whose expectations a judge's answer
could not repeat within the 16 KiB a hook hands back. Its first message is its prompt, with one sentence
naming its files where it has any, as skill-creator's executor is told them.

**A harness hands back a chat's final answer by its `Stop` hook** (the open choice of the ticket):
Claude Code's and Codex's `last_assistant_message`, which purlis's opencode plugin sends too, is
already carried to the app on a line of its own (#1981). The grade watching the chat hears it
there, and nowhere else does a grade's chat's last message go. A chat whose harness hands none is
judged on its folder alone, and its judge is told there is no answer.

**A judge reads only what the chat it judges left, and changes nothing.** Before that chat closes
and its piece goes, purlis reads its plain text files as it ended (at most 32, 8 folders deep, no
dot-name, each read pinned) and keeps them, with its answer, for its judge alone. The judge's
piece is a grade's arm with no skill, marked as a judge's in this machine's state, which no chat
writes, holding `task.json` (the eval's prompt, what success looks like and its expectations),
`answer.md`, `outcome/` and `files.txt`, which lists it, and nothing else. A judge's chat is a grade's chat, held to every
limit of §3, and is denied besides every tool that writes a file, runs a command or reaches the
network (on Claude Code `Bash`, `Edit`, `MultiEdit`, `Write`, `NotebookEdit`, `WebFetch`,
`WebSearch`), every other way to read (`Glob`, `LS`, `Skill`, and purlis's own MCP tools, which
answer from the plane). It starts with `--tools Read,Grep`, so a tool a later release adds is not its either, and the read guard refuses either on a path that
does not resolve inside its folder, links followed, so it reads nothing else even where its harness
would let it.

**One judge chat grades every expectation of one run** (the other open choice): skill-creator's
grader grades a run whole, and a judge per expectation, which Anthropic's "Demystifying evals for AI agents" recommends, would multiply
the judges' chats by up to 32 an eval of the person's spend. The judge's prompt carries
skill-creator's burden of proof: an expectation passes only on clear evidence, and a surface match
fails.

**The judge's answer is taken whole or not at all.** It is skill-creator's `grading.json` shape,
`{"expectations": [{"text", "passed", "evidence"}]}`, parsed as JSON (one code fence around it is
taken off) and taken key by key through an allow-list: every expectation it was asked about, in
order, each `text` the one asked about, each `passed` true or false, each `evidence` text, kept at
most 512 bytes with every control character a space; every other key is left out, and an answer
past 64 KiB is not read. An answer that is not whole and a judge that asks the person something
**fail that run of the eval** (none of its expectations
passed), with a sentence saying why. A judge that reached past what it may read **fails that run too, with the reason in the record**
(the operator's ruling of 2026-11-01). A judge that never ran (not started, stopped, or ended at a
cap) or handed back no answer judged nothing, so the run is no run. Two judges in a row that do not start end the grade, as
two chats do.

**Scoring** (§2): a run's score is the share of its expectations that passed; an arm's outcome
pass rate is that share over every judged run of every eval, as a mean and a sample standard
deviation (skill-creator's benchmark pools them so), and `outcome_delta` is the arm with the
skill's mean less the baseline's, beside the trigger tests' `delta`.

**The cost comes first** (§4): the plan counts each eval's chats and its judges' chats in the
chats it shows and the person consents to, and says how many are judges'. Both are held to the
token cap and the dollar limit like every chat of the grade. A plan that runs outcome evals is
estimated only from a grade that ran some: a trigger test's chat ends once the skill starts, and
says nothing of what a whole turn and its judge spend. Grading after each edit (#3238) accepted a
number of chats and now the evals and their files by digest, so a plan whose outcome evals changed
in any way waits for the person to see it again, even within the chats accepted.

**The record** (§5) gains `report.outcomes` (each eval in each arm, each run's chat, judge,
verdicts and why it failed), `report.outcome_delta`, and each arm's `outcome_pass_rate`,
`outcome_chats` and `outcome_not_run`, all absent in a grade with no outcome eval, so a row
written before reads as it did. A row too long to be read back drops the judges' evidence, then
the verdicts, before it is written.

## Rejected

- **The judge reading the graded chat's piece in place.** Its piece goes when its chat closes,
  and a judge standing in it could change what it grades; a copy taken as the chat ended is what
  the chat left, whatever happens after.
- **The judge's prompt carrying the outcome.** A chat's first message is sent on the program's
  command line and is held to a fixture's prompt's size; files in the judge's own folder hold
  what the chat left whole.
- **Grading in part an answer that is not whole.** A judge's answer the parser cannot take whole
  is a verdict nobody gave; reading what it could would score expectations the judge never
  graded.
- **Staging files that are not text, and reading outputs that are not.** Every file a grade
  stages or reads is held as text, as Try it holds a skill; a binary input, or a binary output a
  judge would need, is a follow-up (#4230).
