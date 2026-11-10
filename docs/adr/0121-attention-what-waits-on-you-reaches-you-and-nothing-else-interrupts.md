# 0121. Attention: what waits on you reaches you, and nothing else interrupts

Status: proposed (2026-11-09). Amends ADR 0119 decisions 2, 3, 5 and 9 and ADR 0038's ✋. Builds
ADR 0076 N4 and N8. Keeps ADR 0097 (the settle guard) and ADR 0112 (run grants) whole.

Quiet as built, and which Expiring asks it can hold open: see [its amendments](0121/), of
2026-11-10.

## Context

The person runs many agents across projects and must unblock them quickly without losing focus
or forgetting any. After ADR 0119 they still missed what stopped their chats, and were
interrupted by what didn't need them:

- **Things that stopped a chat went unseen.** A sandbox consent is a Notice, so it raised no ✋
  count, no dot and no notification, only the status line's "Notices N". The ✋ counted the
  project in front alone. Run grants from `purlis secret exec` timed out twice on 2026-11-09
  unseen, while the core still held them open.
- **Things that didn't need them interrupted them.** "News" Notices popped cards. Notifications
  came per ask.
- **Nothing said what was new.** Only updates had a read state.
- **The same request repeated across chats** and was answered one at a time, forever.
- **Nothing showed the whole fleet.** A chat stuck, looping or idle with work left behind went
  unnoticed until someone looked.
- **A timed-out request vanished**, and how long a request waits differed by kind and harness.

## The decision

### What goes where

1. **Each view answers one question**, and its icon's tooltip says it:

   | Place | The question | What it holds |
   |---|---|---|
   | **Asks** (✋, top of the right bar) | Do I need to answer something? | every **Blocking ask** |
   | **Updates** (bell) | Did my work finish? | results of the person's own chats (Mine); the project's changes (Project) |
   | **Notices** (warning sign, Lower icons, opens in the Panel) | Is something about my setup wrong or worth knowing? | the project's and the machine's Notices |
   | **All chats** (a mode of the left Chats sidebar) | What is everything doing? | every chat in every open project, with its **chat state** |

   Nothing is listed in full in two of these. Elsewhere it has at most one marker: a chat's pane
   band or sidebar row says "waiting on you" and links to the ask, never a second form; a
   workspace chip shows a dot, never a card; the status line no longer repeats counts the right
   bar shows.

2. **A Blocking ask is defined by what it does.** Anything that stops one of the person's chats,
   or a `purlis` command, until they answer is a Blocking ask, whatever its source: a
   permission, a question, a dispatch grant, a secret, a run grant, a browser site or login, a
   sandbox block or consent, a command's confirmation in the window, a budget reached, a
   suspected loop. A decision that stops no chat (a sandbox policy, a persona grant, a report
   draft) is a Notice. A teammate's request stays under Updates › Project, uncounted, unless it
   stops the person's own chat (ADR 0119 decision 1). A chat that can't tell purlis it waits (a
   shell, a harness without hooks) is never counted; All chats shows it as **Can't tell**.

### Three levels, one signal each

3. | Level | Signal |
   |---|---|
   | **Blocking** | the ✋ count, one system notification per chat, the Dock badge |
   | **Should know** | the bell's count; a notification only for a failure, or for a finish after more than 5 minutes while the window was not in front |
   | **Just so you know** | the **New** marker only. The Notices dot is red for something broken and amber for a decision that stops no chat |

4. **The ✋ counts every open project's Blocking asks**, with "+N elsewhere" for those outside
   the project in front, and the Dock badge shows the same total. Asks opens on "all projects";
   a filter narrows the list but never the ✋.

### Order and speed

5. **Order by facts, never by a priority someone sets.** First an **Expiring ask** with its
   countdown, then an ask that blocks other chats ("blocks 3 chats"), then by longest wait.
   Requests from one chat stay together, and each row is tagged with its project and workspace.
   Each row's reason line says why it stands there. A new ask takes its place in that order and
   never jumps to the top or changes the open view (ADR 0119 decision 4).

6. **Answer once for all.** Requests that are exactly identical (the same kind and the same
   host, command, or tool and argument) from several chats, in any project, are one row: "3
   chats want to reach api.example.com", with Allow for all, or open it to answer each.

7. **Ask less.** After the same answer to the same request twice, the row offers a rule: this
   chat, this project (committed, so teammates see it under Updates › Project) or this machine.
   The default is the narrowest that would have prevented the asks. Asks links to "Rules I
   made" to review and undo them. Rules are offered only where ADR 0112 and its kind already
   allow one (hosts, reviewed programs, permission rules), never for an arbitrary command.
   Repeated Don't allow leads to a cool-off (#5285).

8. **Risk is shown and slows things down** (#1046). An ask that pushes to a protected branch,
   reaches a host that looks like production, or writes outside the project shows its risk. It
   is never grouped, never offered as a rule, and needs a deliberate press: keyboard triage does
   not advance onto it.

9. **Keyboard triage.** ⌘⇧I (Ctrl+Shift+I) opens Asks with the first Answer focused. Answering
   moves to the next, and there are keys for later, go to chat, snooze and stop. Focus returns
   where it was when you're done. The settle guard still applies to every press.

### New, notifications and quiet

10. **New means seen, not received.** An ask, Notice or update is **New** until its row has been
    visible in an open view for about a second, or its chat has been on screen in a focused
    window. A New row has an accent bar; its icon, chip or status item has a "new" pip. Asks
    and Notices gain this seen state, kept per machine, as updates already have.

11. **Where you're looking.** An ask from the chat on screen in a focused window doesn't notify
    and isn't New, but is counted.

12. **Notifications.** One per chat, updated in place ("steward 3: 3 things wait") and removed
    when all are answered. When 3 or more chats ask in a burst, one notification for all ("3
    chats in 2 projects need you"). Each names the project, chat and ask. A click brings the chat
    forward with the Answer open. No notification has an Allow or Deny of its own, because that
    would bypass the settle guard. No sound, except an optional one for an Expiring ask about to
    fail.

13. **Quiet.** One idea, not two: on a schedule (today's quiet hours), as "Quiet now", and
    automatically while the person types in a chat. It follows macOS's own Focus. While Quiet,
    the ✋ still counts and notifications wait. An Expiring ask is held open past its time
    instead of timing out. When Quiet ends, one summary row at the top of Asks ("While you were
    focused: 3 asks, 2 finished, 1 failed"), each part a link, which goes once it's all seen; one
    notification if the person is away.

14. **Snooze** keeps "until it asks something new" (ADR 0119 decision 9) and adds "for 1 hour"
    and "until tomorrow", whichever comes first. A snoozed chat neither counts nor notifies.

### Timeouts and commands

15. **A timed-out ask stays answerable**: "Timed out. Answer, and the chat is told to try
    again." Answering reaches the chat.

16. **A waiting `purlis` command says where**: "waiting for the person: Asks (✋) in the purlis
    window, project smart-ide, for up to 90 seconds".

### The fleet

17. **Chat states, the same everywhere** (sidebar row, pane tab, All chats, Asks): **Working**,
    **Waiting on you**, **Waiting on a chat**, **Stuck?**, **Done**, **Failed**, **Idle** (finished
    with work left behind) and **Can't tell**, each with one icon and colour.

18. **All chats** lists every chat across projects: state, time in it, project and workspace,
    and last line. It is sorted by Waiting on you, then Stuck?, then the rest. Several chats can
    be stopped, snoozed or hibernated together. It shows each project's asks this week, the
    median time to answer, and how many asks rules saved.

19. **Stuck? is a question.** Working with no new command or output for 15 minutes, the same
    command failing 3 times in a row, or a turn longer than 60 minutes (each a setting) marks a
    chat Stuck? and makes it a Should-know item. "It's fine" clears it until the chat changes
    state.

20. **Budgets and runaways are Blocking asks** (ADR 0076 N4, N8). A chat that reaches its
    budget, or is found looping, is paused and asks: "Budget reached: Continue, Raise the
    budget, Stop", or "It's fine, continue" or "Stop".

21. **The same actions on every chat row**, in one order: Answer (when it asks), Go to chat,
    Snooze, Stop.

### Settings, empty states, access

22. **One Settings page, Attention**: Quiet (schedule, Quiet now, while typing); the Stuck?
    thresholds; notify when finished, after N minutes; the Expiring sound; notifications per
    source, for Should know and Just so you know only, since a Blocking ask can never be muted.

23. **When nothing waits**: no number on the ✋; Asks says "Nothing waits on you · 7 chats
    working · 1 stuck?", each part opening All chats filtered.

24. **No tour.** The tooltips carry the questions; the ✋ pulses once the first time a Blocking
    ask arrives while Asks is closed.

25. **Screen reader and keyboard.** A new Blocking ask is announced once, politely. Counts are in
    each icon's label; Stuck? and risk are spoken on the row; every action has a key.

## Consequences

- ADR 0119: decision 2's dot is red or amber; decision 3's ✋ counts every project; decision 5's
  Asks opens on all projects; decision 9's snooze gains a time.
- ADR 0038's ✋ counts consents and budget pauses as well.
- Asks and Notices keep a seen state; the core keeps rules made from offers, and ask counts and
  times to answer per project.
- A sandbox consent is listed as an ask as well as a Notice; one answer settles both.
- A timed-out ask needs a way back to its chat for each harness.
- The scenario specs find every Blocking ask from the ✋ alone, with no view opened first, and
  each flow counts its interruptions against a ceiling (#1079). The specs that shipped ADR 0119
  opened views directly and so missed the unseen asks.
- Not chosen: priority levels or an "urgent" flag (they decay into noise); answering from a
  notification (it bypasses the settle guard; #567 IB-14's actionable notifications are dropped);
  leaving the on-screen chat's asks out of the count (a count that changes with where you look
  reads as a bug); a separate focus mode (the name is #2921's, and Quiet covers it).
