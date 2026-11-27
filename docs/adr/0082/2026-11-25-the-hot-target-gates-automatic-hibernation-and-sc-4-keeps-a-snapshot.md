# Amendment, 2026-11-25: the hot target gates automatic hibernation, and SC-4 keeps a snapshot (SC-4, #1391)

Proposed by the SC-4 spec (#1391) from the operator's decisions of 2026-11-25. It takes effect
when the operator accepts this file.

**§4's rule changes by one clause.** §4 said *"The hot target never refuses, queues or hibernates
a chat the operator starts"* and *"Hibernation stays as ADR 0076 §5 defines it: idle past the
threshold ... The hot target does not shorten the threshold."* Now: **the target never refuses or
queues a chat the operator starts, and hibernates only a rested chat** (ADR 0076, amended the
same day), never one that is working, asking, paused or looked at. The target still does not
shorten the threshold. **What it does is gate.** The app hibernates a rested chat by itself
**only while the device's hot chats are more than its RAM class's hot target**, and stops the
moment they are not.

- **Under the target, a rested chat stays hot, however long it rests.** The timer alone never
  hibernates one. A device with room keeps every chat ready.
- **Over it, the longest-rested chat goes first**, one at a time, and the count is read again
  after each. The app stops at the target, and so does not empty the device of hot chats.
- **If every hot chat is busy, nothing happens.** Telling the person is SC-19's.
- **The target still refuses nothing.** Starting, resuming and waking a chat is admitted past it
  (§4's first paragraph, unchanged). A hibernated chat woken while the device is over its target
  is a hot chat like any other, looked at, and so not rested for the full time.
- **The operator's Hibernate is not gated.** It is theirs, on any chat that can be hibernated,
  over the target or under it.
- **SC-19's warning offers the same act.** It is not changed here.
- **The rule reads a RAM class and a hot count where the chats run.** Neither is in code yet:
  one SC-4 ticket reads the class when the app starts (§3's formula and table; this
  replaces §5's *"when the host starts"*) and stores nothing.
  A runner's chats are not covered: their snapshot would be on the runner (a follow-up).

## The snapshot's store

§5 left *"the hibernated chat's scrollback snapshot"* to SC-4, with its store and tier. SC-4
decides:

- **What it is:** the chat's screen and scrollback as the escape sequences that draw them into a
  blank terminal, the engine's snapshot, as a skill grade's kept chat and a dispatched task's last
  screen are (`docs/plane-format.md`, `skill-grade-chats`, `app/chat-screens/`). Written when the
  run is hibernated by the app's act; read when its pane opens.
- **Masked:** before it is written, by `secrets::handedto`, as `app/chat-screens/` is (#6309). A
  screen that cannot be masked is not kept, never kept unmasked.
- **The bound:** at most **1 MiB** on disk (spec D3), cut at a line the engine drew and never
  inside an escape sequence, the oldest first. SC-5's bound on a hot chat's scrollback is a
  separate ticket. While the alternate screen is open the snapshot shows it alone (the engine's
  limit).
- **The store and tier:** `.charter/app/hibernated-screens/<ULID>.screen`, **Clone state** like
  its sibling, `0600` in a `0700` folder, through no link. **Not rebuildable**: the process that
  held the grid is gone. Losing it loses only the kept view; the chat still resumes by id.
- **It is removed** when the chat is closed or killed, when its run is no longer hibernated and
  has drawn again, when its workspace is removed, and by the open's sweep when no hibernated reopen entry names it.
- **(proposed)** **A chat hibernated by a quit, the grace period or a host crash has none** (nothing
  took one), and its pane says so where it shows one; a snapshot that cannot be taken or masked
  never stops a hibernation. A still-hibernated chat's snapshot is also removed after 30 days.
- **Its cost** is M5 (native cost of a hibernated chat, 1 MB) and D3, measured by SC-8's `stress`
  with 150 hibernated chats.

The store and its `**Tier:**` line go in `docs/plane-format.md` with the ticket that writes it
(CLAUDE.md, *A new store names its tier before it ships*).
