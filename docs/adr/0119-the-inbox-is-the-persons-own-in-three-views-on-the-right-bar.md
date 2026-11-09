# The Inbox is the person's own, in three views on the right bar

**Accepted 2026-11-07**, the operator's decisions in two grilling sessions of that day, each
checked against a throwaway prototype on branch `prototype/inbox` (never merged): a logic demo
(`app/src/inbox.prototype.html`) and the variants of the right side
(`app/src/inbox-variants.prototype.html`), of which the operator chose variant A. **Supersedes
in part [ADR 0038's "one list"](0038/2026-10-11-02-one-list.md)** and **amends
[ADR 0038's "the Inbox heads the right side"](0038/2026-10-11-01-the-inbox-heads-the-right-side.md)**:
the Inbox is no longer one view. What those amendments say of the ✋ (it counts asks alone), of a
pane's own Notices, of the window's own lines, of the retired ✋ list and of system notifications
stands.
**Amended by [ADR 0121](0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)**
(2026-11-09): decisions 2, 3, 5 and 9.

## Where purlis was

- **The Inbox was one view with one list, and it was never quiet.** Asks, chats waiting in other
  windows, every Notice and every update stood in one column, so what waited on the person was
  found by reading past what did not.
- **A fresh clone opened onto other people's history.** The teammate Notices
  (`ProjectHostsNotice`, the presets Notice, `ProjectDispatchNotice`) compare what the project
  commits with a record of what this machine has seen (`hosts_seen`, `presets_seen`, grant
  answers). On a new machine that record is absent, so every host change, preset change and
  dispatch grant any teammate ever made stood as new, with no filter by who made it or whom it
  is for. Each doctor finding about the committed plane stood as its own line besides.
- **Read and dismissed are this machine's**, so nothing a person had settled elsewhere carried
  over, and the clone started with nothing settled.
- **"Dismiss all" was for updates only.** Notices were dismissed one at a time, and asks only by
  Ignore on a whole chat.

## The decision

1. **The Inbox is the person's own.** It lists what the person must act on or should know:
   asks they may answer, results of their own chats, Notices about their machine and their work.
   A line about another person (a grant waiting on a teammate's answer, a teammate's runner key)
   is not listed. What teammates changed in the project is listed only under **Project** in
   Updates.
2. **The Inbox is three views on the right bar**, each with its own icon, which never moves and
   stays when its view is empty:

   | View | Where on the bar | Icon and count | What it lists |
   |---|---|---|---|
   | **Asks** | first, at the top | the hand; the asks, in the needs-you colours | every ask, grouped by chat, the longest wait first; a chat waiting in another window or project under "In other windows" |
   | **Updates** | second, at the top | the bell; the unread **Mine** lines, in the plain count's colours | a switch of two lists: **Mine** (**From your chats**) and **Project** (**From the project**) |
   | **Notices** | first of the Lower icons, before Memory, Todos and Sessions | a warning sign; a red dot only while a Notice says something needs fixing, never a number | the person's Notices, what needs fixing first; every doctor finding together as **one** line that opens to the list |

   Notices opens in the right half of the Panel (#1738), so it stands under Asks or Updates rather
   than in their place, and the status line's Notices item opens it too. Personas and Vaults keep their
   places after Updates.
3. **The ✋ counts asks alone**, as ADR 0038 says, and opens Asks; so does ⌘⇧I (Ctrl+Shift+I
   elsewhere). Updates and Notices are rows of the palette, with no keys of their own. A finished
   task waits on nobody, so it never rings the ✋.
4. **Nothing arriving changes the view.** A new ask raises the hand's count and its system
   notification, never the open view.
5. **The scope switch** stands at the top of Asks (all projects, this project, this workspace)
   and of Updates (this project, this workspace: a window holds no other project's updates, so
   Updates has no "all projects"). Notices are always the project in front's.
6. **A message is a row of its view, not a boxed card.** A tinted round icon says its state
   (waiting, finished, failed or needing a fix, a report), its title is the chat or the thing,
   its time stands at the right, and its ways out are text links under it, with × on hover. What
   the existing cards say stays word for word.
7. **An ask is answered where it stands, behind an Answer link.** Answer opens, in the row, the
   reply box for a question or the options for a decision (Allow once, Allow always, Deny), with
   the cursor in it. An ask only its own place can answer (a prompt in a terminal, a browser
   sign-in) has Go to chat alone. A secret ask keeps its form behind Answer: the Inbox row is the
   only place it can be answered, and its value never goes into a chat. A long message shows three lines and
   "Show all · N lines".
8. **A result opens in place.** A **Mine** line opens to show its report or the reason it
   failed, and opening it marks it read; Go to chat still goes to the chat.
9. **Every list can be cleared at once, by its own rule:**
   - **Snooze all** on Asks, after one confirmation in the view, puts each chat's asks away until
     that chat raises an ask it did not have when snoozed; then the chat comes back with all its
     asks. It answers nothing and has no timer. A snoozed chat still says "N waiting" on its row.
   - **Dismiss all** on **Mine**, **Project** and Notices is each line's own Dismiss, done to
     every line: an update goes for its day, a Notice until its cause changes. An Undo stands
     for a few seconds after.
10. **The first-open baseline.** The first time a plane opens on a machine with no seen record
    for it, everything already there counts as seen and goes to **Project**, read, except the
    open asks the person may answer. This covers the sandbox hosts and presets a teammate changed, whose seen records
    it writes, and the doctor findings; a finding that appears after that is new. A dispatch
    grant or a browser site a teammate committed is **not** counted as seen, since that would
    put it in force on this machine: it stays under **Project**, never counted, until the
    person answers it. A machine that already has a seen record is
    never baselined, so updating purlis clears nothing.
11. **Read, dismissed, snoozed and the baseline are this machine's**, in purlis's data home, and
    never in the project: settling a line never settles it for a teammate.

## Why

- **One icon per kind of attention.** An ask blocks a chat, a result is news about the person's
  own work, and a Notice is a standing fault. Each has its own place and its own kind of count,
  so only the hand is loud and nothing competes with it.
- **Teammates' changes behind a switch, not an icon.** They are the least pressing, and as an
  icon of their own (the prototype's variant B) they stood on the bar for good without ever being
  counted. As sections in one view (variant C) they pushed the person's own results down.
- **Notices at the bottom.** A Notice stands until it is fixed, beside the other views a person
  opens when they choose (Memory, Todos, Sessions), and the Panel lets it be read without leaving
  Asks.
- **The leak was a missing baseline, not missing filters.** A new machine had no seen record, so
  the whole project's past was new. Setting the record at first open, and listing teammates'
  changes apart from the person's own, makes a fresh clone open with only what waits on them.
- **One clear-all per list, never one for the Inbox.** Snoozing asks must not hide a chat that is
  blocked, so it ends on the chat's next ask. Dismissing an update or a Notice is already safe,
  since a Notice comes back when its cause changes.

## Consequences

- `Inbox.tsx` splits into the Asks, Updates and Notices views, each a view of the right bar
  (ADR 0038, 2026-10-10-02). `InboxElsewhere` becomes the "In other windows" group of Asks.
  `inboxUpdates.ts` sorts each update kind into **Mine** or **Project**. The rows keep
  `InboxRow`'s and `InboxReplyCard`'s words and change their frame.
- The teammate Notices move from `standing` (`PlaneView.tsx`) to **Project**, and their seen
  records are written at first open. `DoctorNotices` and the doctor updates fold into one
  Notice line.
- **Snooze all** keeps, per chat, the asks it put away, in the inbox store
  (`crates/purlis-core/src/inboxupdates.rs`) beside read and dismissed, and the first-open
  baseline is a per-plane record beside the seen records. Both are **Machine, device-bound**
  (ADR 0069), and `docs/plane-format.md` names them before they ship.
- Notices joins the right half's views (`RIGHT_HALF_VIEWS`) and the Inbox's place in the
  sidebar's (`RIGHT_SIDEBAR_VIEWS`) is taken by Asks and Updates. The layout file remembers
  which view each place shows, as it does today.
- **Inbox**, **Update** and **Notice** in `CONTEXT.md` are amended, and **From your chats**,
  **From the project** and **First-open baseline** are added.
