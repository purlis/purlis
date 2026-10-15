# Amended and accepted (2026-10-07)

The operator accepted this record on 2026-10-07, with the changes below. They were decided with
the operator in a grilling session that day (40 questions, every recommendation agreed, and the
limits made configurable at three levels), and they are the spec of milestone M62 (#1434). #1435
records them here.

**How to read the record from here on.** The text above is kept as it was proposed. Where it
and this section differ, this section holds. Everything this section does not name stands as
written. Two words change throughout: the proposal's *dispatcher* is the **asking chat**, and
the chat a dispatch starts is the **persona chat**. "Delegation" is not a term.

**What it changes in the code.** This record is still ahead of the code. M62's tickets build
it in steps, and until dispatch ships, `purlis handoff` works as it does today. One rule ships
with the acceptance: `purlis persona use` is refused inside a chat (change 1).

## 1. A persona is never a harness sub-agent

A **persona** is a role a chat runs as for its whole life. The proposal left the older reading
beside it, in which a persona was also handed to the harness as a sub-agent of some other
persona's chat. That reading is withdrawn.

- **A chat's persona never changes.** It is fixed when the chat starts. `purlis persona use`
  inside a chat the app started is refused, writes nothing, and names the two ways forward: the
  operator allows the vault on the chat's tab, or the chat dispatches to that persona.
- **Work for another persona goes to a chat of its own, by dispatch.** A sub-agent runs inside
  the asking chat's process, so the tool gate, the vault and the hosts all follow the chat's
  persona and never the sub-agent's. Only a chat really holds what its persona was given.
- **purlis stops writing a sub-agent for each persona.** It removes the ones it wrote and says
  what it changed, and a sub-agent call named for a persona is refused with the dispatch route.
  M62 does this after waiting and follow-ups ship.
- **A harness's anonymous helpers stay.** A helper sub-agent carries its chat's persona and is
  a child run, as before. It can never dispatch: only the chat itself does. That withdraws the
  part of §10 that had AC-5 lift guard A7 per harness.

[ADR 0072](../0072-charter-has-five-concepts-and-every-other-word-belongs-to-one-of-them.md) and
[ADR 0066](../0066-a-chat-is-a-ulid-a-run-is-a-stretch-of-its-conversation-and-a-device-is-random.md)
each carry a note of this.

## 2. A task chat is listed, never hidden

§1 and §7 open a `task` dispatch headless. That stays, with its meaning fixed: **headless
means "has no tab yet", and never "hidden"**.

- A task chat is listed under its asking chat, with its persona's icon and its state. Pressing
  it shows it inside the tab of the session that asked for it, and adds no tab (amended
  2026-10-08, V100-31, #1486: it used to become an ordinary tab, whose close read as ending
  the session). A handoff opens as a tab, as before.
- Listing ships with dispatch itself, so §7's "not before AC-12" no longer holds a task chat
  back.
- A persona chat stays open after it reports, marked as reported, until the person or its asking
  chat closes it. Closing an asking chat asks once what to do with the chats it started that
  are still running.
- A needs-you mark on a persona chat rolls up to the chat that asked and to the workspace's tab,
  so a collapsed list cannot hide it. A report goes to the asking chat and is not a needs-you
  item.

## 3. Consent is the dispatch grant

§3 made a person's yes per dispatch the default, and a dispatch grant the exception. **The
grant is now the only consent**, and it replaces three things: the per-dispatch ask of §3, the
harness's permission prompt for `purlis handoff`, and the held-grants rule.

- **A dispatch grant is the person's rule that one persona may dispatch to another.** It names
  the pair. The shape §3 gave it (workspaces, modes and caps of its own) is dropped: limits are
  settings (change 4).
- **It has three levels:** this chat, gone when the chat closes; me on this machine, never
  committed; and everyone in this project, committed, with a one-time Notice to each teammate
  when the project's grants change.
- **The first dispatch of a pair raises a Notice** that shows that first brief in full and
  offers the three levels. With a grant, the persona chat starts with no prompt. Without one it
  does not start. Nothing a chat sends can make a grant.
- **Some dispatches need no grant:** one to the asking chat's own persona, and one the person
  makes themselves from a chat's tab, whose report goes to that chat marked as started by the
  person.
- **Policy can lock** one pair, or all dispatch.
- **An unattended chat dispatches only under a grant that already exists** for the person or
  the project. A missing grant is a refusal there, and the persona chat never inherits the
  bypass.
- **The harness prompt is retired as consent.** Dispatch goes through purlis's own tool and
  command, answered by the app, the same on every harness. That replaces §1's route over the
  MCP Tasks extension. The handoff guard stays where it refuses a helper sub-agent.
- **The held-grants rule is retired for dispatch.** [ADR 0067](../0067-a-chat-runs-in-a-sandbox-charter-compiles-for-its-harness.md),
  as amended on 2026-10-06 (D-1362-5), started a handed-off chat on the asking chat's narrower
  grants until the person allowed its own. A persona chat now starts with its own persona's
  hosts and vaults, because the grant is the person's yes to exactly that.
- **A persona chat's sandbox is the project's for its own persona.** It never takes the asking
  chat's per-chat grants, its opt-out or its permission mode. A project with no sandbox gives
  neither chat one.

§5 stands, and is the other half of this: **a brief is a request from another chat, never the
person's word.** The persona chat is told who asked. Its own guards and charter apply, and every
command that asks the person still asks, in the persona chat's own tab. Nothing in a brief
approves anything ahead of time, and nobody approves a brief. A report is data to the asking
chat, never instructions: an outcome (done, blocked or failed), its text, what changed, and where
its session record is. When the person types in a persona chat, its report says the operator
stepped in, and not what was typed.

## 4. The limits, and their three levels

§8's three caps become these limits. Each is a setting, not a part of a grant.

| Limit | Default | Past it |
|---|---|---|
| chats one asking chat has running | 6 | the dispatch is refused, naming the limit |
| live chats in one lineage | 16 | the same |
| depth of a chain of dispatches | 3 | the same |
| messages a minute between one pair of chats | 10 | the message is refused |
| chats one persona may dispatch | no cap | refused once a cap is set and reached |
| chats that may run as one persona at once | no cap | the same |

- **Three levels set them: the project, a workspace and a persona.** The most specific wins,
  persona over workspace over project, and a level that sets nothing inherits.
- **Policy is a ceiling** on every one. A person's own override on their machine may only lower
  a limit.
- **0 switches dispatch off** at that level.
- **Two rules are fixed** and no setting moves them: no persona appears twice in its own chain,
  and depth never passes 8.
- **They are kept in the project's committed settings**, a persona's limits included. A chat
  can edit its own persona's definition, so a limit kept there would be one a chat could raise.
- **A new start waits** while the machine is short on memory. Built in #1467: the last step of
  the decision, after the grant. "Short" is the operating system's own verdict (macOS's
  critical memory pressure level; Linux's pressure stall information, else the memory
  available), and a machine purlis cannot read holds nothing. The dispatch is held, the asking
  chat is told it waits (on the person's Allow too, #1617), the asking chat's row says how many
  of its dispatches wait, the Dispatches tab lists it, and it is decided again once memory
  frees; after 10 minutes still short it starts nothing and the asking chat is told. The
  person's own dispatch from a chat's tab does not wait (D-1467-4, delegated).

## 5. Messages travel along the lineage only, in the first version

§4 let a chat message another chat outside its lineage over a message link a person made. The
first version has no such message.

- The asking chat may wait for a report or carry on, which is the default, and is told when the
  report lands. It may send follow-ups to a persona chat it started, and cancel it. A cancelled
  chat is asked for a short report.
- The persona chat may send progress notes and questions back. A question pauses it.
- **A question only the person can answer goes to the person, in the persona chat's own tab,**
  and never through the asking chat.
- No message passes between siblings or unrelated chats. Message links and the mailbox wait
  (change 7).

## 6. Claude Code and Codex first

The first version dispatches from and to chats on Claude Code and Codex, across the two as
well. A persona chat runs on the profile its persona names, else on the asking chat's, and the
asking chat may name another of the project's profiles. opencode follows when its chats ship.

## 7. What is out of the first version

Each of these keeps a ticket on the M38 map, and the text above stays the proposal those
tickets start from:

- delegated approvals, one chat answering another's asks (AC-11). §5's rule for it stands: a
  delegated answerer is never the asking chat, nor any chat in its lineage;
- triggers and schedules that dispatch on their own (AC-9);
- a coordinator persona (AC-8);
- budgets that stop a lineage at a cost limit. Cost is shown, not enforced;
- peer messages between siblings or unrelated chats, message links and the mailbox (AC-3);
- opencode;
- any change to what a harness's anonymous helper sub-agents can do, beyond refusing them a
  dispatch and a persona's name.

## The six open questions, as ruled

| # | Question | Ruling |
|---|---|---|
| 1 | The dispatch grant | Accepted, and made the only consent (change 3) |
| 2 | purlis's own consent, not the harness's prompt | Accepted as the grant's Notice. There is no ask per dispatch (change 3) |
| 3 | The new words | Accepted: Dispatch, Task, Handoff, Report, Lineage, Dispatch grant and Headless chat, with **Asking chat** and **Persona chat** added. Peer message, Mailbox and Message link wait with change 7 |
| 4 | The cause `dispatcher` | Accepted as written |
| 5 | The caps' initial values | Replaced by the limits of change 4 |
| 6 | AC-11's lineage exclusion | Accepted as written. AC-11 itself is out of the first version |
