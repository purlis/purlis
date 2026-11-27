# Amendment, 2026-11-25: a hibernated chat is a state, and Hibernate is a row action (SC-4, #1391)

Proposed by the SC-4 spec (#1391) from the operator's decisions of 2026-11-25. It takes effect
when the operator accepts this file. It answers #6013's unchecked box (*several chats can be
stopped, snoozed or hibernated together*) and #2278 (a hibernated chat reads so everywhere).

**Decision 17 gets a ninth chat state, Hibernated.** A chat whose current run is `hibernated`
(ADR 0076) is **Hibernated**, with its own icon and colour from the theme's semantic tokens, the
same on the sidebar row, the pane tab, All chats and Asks. It is not Working, Waiting, Stuck?,
Done, Failed or Idle: its program was ended on purpose and its conversation is kept to be resumed.

- **Its row is dimmed** and **sits at the end of its list**: after every other state in the
  sidebar and in All chats, which decision 18 sorts Waiting on you, then Stuck?, then the rest.
- **It never asks, never counts toward the ✋ and never notifies**, and **Stuck? never applies**.
  A chat hibernated by a host crash (ADR 0076 §2's `host-crash` row, ADR 0068 amended, #2278) is
  Hibernated too, and so is one that quit hibernated.
- **Opening a chat the app or the operator hibernated shows its kept scrollback and a Resume
  button**, or says none was kept and still offers Resume. A chat hibernated by a host crash has no
  kept scrollback, and says so, with the Resume it has today. Typing a message in it resumes it as
  well. All are ADR 0076's wake of the same chat. **(proposed)** A chat that quit hibernated
  follows SC-20's reopen-on-show until the operator rules otherwise (SC-4 spec, ruling 3).
- **(proposed)** Hibernating does not mark a finished turn seen: ADR 0076 §9's needs-you mark
  stays on the row beside Hibernated.

**Decision 21's order gains Hibernate: Answer (when it asks), Go to chat, Snooze, Hibernate,
Stop**, with a key like the other actions (decisions 9 and 25). It appears on every row of Asks,
All chats and a Stuck? item, and in decision 18's bulk actions beside Stop and Snooze. It works
only where it can: on a row that asks or is Stuck? it is shown **disabled with its reason**. The
row says why in words, per `docs/ui-copy.md`:

- the chat is not at a turn boundary (working, asking or paused);
- its harness cannot resume a conversation by itself (ADR 0076 §5, X35). Such a chat is also
  *never* hibernated automatically, and its row says so;
- it has a dispatched task of its own still running, or it is a task whose report is owed or
  whose parent is waiting on its report;
- its harness's own work purlis cannot see: it is never hibernated *automatically*, and can be by
  hand with a warning;
- it is an observed chat (ADR 0076 §7), or a runner's chat.

A bulk Hibernate does what it can and lists the chats it left, each with its reason. Chats with
own work are asked about once, in one ask that lists each and what ends; declining leaves them
hot (ADR 0076, amended the same day).
