# A task question goes to its asking chat, and the person chooses where to answer

**Accepted 2026-11-02**, by the operator's rulings of that day. It reverses the rule in
`dispatchtalk.rs`, "A question for the person never enters here": a question a task means for
the person now travels the same route as any other.

## Where purlis was

A task had three ways to ask: `dispatch_ask`, `ask_operator` and its harness's own question
prompt. Which one it used was the model's reading of one sentence in a tool description, so two
tasks with the same question could reach different places.

## The decision

**The route is code, and where the person answers is their setting.** The two are separate
choices.

- A **task question** always goes to the task's asking chat first, which answers it or **passes
  it on** to the person. A task cannot choose otherwise, a dispatch cannot override it, and a
  question prompt or a plain `ask_operator` from a task is redirected to the same place. Where
  the app's record shows no asking chat to answer, it goes straight to the person.
- **Where you answer** is one remembered setting, "from the top" (the default) or "from each
  session". It moves what the person sees, never who is asked, so it can change while questions
  are open. The person can always open a task and answer there, and the first answer wins.
- **An asking chat's answer is never the person's word.** The task is told who answered.
- **Only task questions follow this.** A permission, a secret, a dispatch grant, a sandbox block
  and a browser takeover stay the person's alone (ADR 0080 §5).

## What stays judgement

What an asking chat must pass on (security, cost, anything public, anything amending a ruling)
is a written rule in its briefing, not enforced. Code cannot classify a question's meaning, and
a keyword filter would look deterministic while missing cases and blocking harmless ones.

## Considered options

- **A field on the question saying who it is for.** Rejected: the model would still choose.
- **Every question straight to the person.** Rejected: the person would answer what an asking
  chat could settle in seconds.

## Consequences

Open questions have to survive a restart, because the route now depends on them, where they were
held in memory before. A harness with no adapter to redirect its own prompt falls short of this
rule until it gets one: its prompt stays in the task's pane and the **Inbox**, and the window says
so.
