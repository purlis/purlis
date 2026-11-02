# Rulings on map #4575 (Chats, tabs and panes: a chat isn't a tab), all accepted 2026-11-02

## #4576 charting decisions D1-D9
## Answer (operator, 2026-11-02: yes to all)

1. **Where the chosen chat goes:** picking a chat replaces what the focused pane shows, and the replaced chat keeps running. A chat already on screen elsewhere is jumped to, never shown twice.
2. **Close:** closing a pane only clears it, and the chat keeps running in the sidebar. Ending a chat is a separate action on the chat. The danger cue on close (#3295) goes.
3. **A separate tab appears only for:** an explicit "Open in new tab", a view (Settings, a persona card: the 2026-09-23 ruling), or a chat dragged onto the strip. A split stays in its tab. A pin keeps a tab but doesn't create one.
4. **Chats started without the person:** dispatched tasks and handoffs start in the sidebar only and never take a pane. A chat the person starts with `+` takes the focused pane.
5. **Sidebar:** the "This tab" filter goes; Workspace and All stay.
6. **Layout:** the existing tree of rows and columns, not a free grid. Drag: a pane's edge splits, its centre swaps, the strip makes a tab, from the sidebar into any of those. "Tile all" gives an even grid.
7. **Notices:** minimised is one icon with a count; expanded shows them all. A consent or a blocking question stays open even when minimised, and Dismiss all skips it. The minimised or expanded choice is remembered across all panes.
8. **Upgrade:** tabs with a split, a view or a pin stay. Plain one-chat tabs close, and those chats live in the sidebar. No setting for the old behaviour.
9. **Commands:** each lives on what it acts on. Chat actions are in the pane header, the sidebar row's menu and the palette, with the same list and wording. A tab's right-click holds only tab things. Pane actions stay split, stacked and close, plus drag and "Tile all". Nothing lives on one surface only, and a test holds that. Most chat actions sit behind one "⋯"; which are shown directly is the prototype's.


## #4588 edges
## Question

What happens at the edges the charting decisions left open?

1. A window with no pane after the upgrade: what do `+` and a sidebar press act on?
2. Where does a blocking ask from a sidebar-only chat appear?
3. How is a handoff to another workspace announced, now that its tab was its only announcement?
4. Shells and editors: are they chats here, and do they follow the same rules?
5. Does pressing a task replace the focused pane, given `CONTEXT.md:849` says otherwise?

## Answer (the person, 2026-11-02: yes to all eight)

Round one answers the ticket's questions 1 to 3 and the view case. Round two answers questions 4 and 5 and the workspace case. Numbers below are this ticket's, and D1 to D9 are the charting decisions.

### Which pane a press acts on

1. **A pane always exists.** Each workspace, and the plane root's strip, always keeps at least one tab, so a focused pane always exists and `+` and a sidebar press always have something to act on. Closing the last pane of the last tab leaves an **empty pane** with a New chat button, and the tab stays. On the first launch after the upgrade, each workspace's tab shows the chat that was in front there where that's known, otherwise an empty pane.
2. **A view isn't replaced by a press.** This narrows D1. If the focused pane holds a view (Settings, a persona card), the chat goes to the most recently used pane that shows a chat or nothing, and that pane's tab comes to the front. The view stays where it was left.
3. **A chat in another workspace brings its workspace forward.** The chat replaces the focused pane of that workspace, and the pane the person was looking at is untouched. ADR 0036's invariant holds: the tab in front is always on the strip that's drawn.
4. **A task lands like any chat.** A press replaces the focused pane, or jumps to the task if it's already on screen. This replaces "pressing the task switches that tab to it" in `CONTEXT.md`'s Task entry. The breadcrumb path stays, and pressing it walks up the chain in the same pane.

### What arrives without a pane

5. **A blocking ask from a chat in no pane never takes a pane or focus.** It shows on the chat's sidebar row, on the rows above it, on the title bar's ✋ and in the workspace chip's count, and the person answers it in the Inbox. The red count on a tab moves to the row. If the chat is on screen, its pane band also shows the ask and D7's "stays open" applies. A task's questions still go to its asking chat first (#4551).
   - Checked in the code: the Inbox doesn't answer everything directly. A prompt in a harness's terminal that no hook can carry is listed with Go to chat (`CONTEXT.md`, Ask), and that press follows rule 1 to 4, so it replaces the focused pane. It was the person's press, so D4 isn't broken.
6. **A handoff is announced in three places, and takes no pane.** This applies to every handoff, including one within the workspace in view.
   - A card hangs from the target workspace's title-bar chip: "<chat> arrived from <asking chat>", with Go to it. It never takes focus, and pressing elsewhere leaves a dot on the chip (the chip rules of spec #3647). With no chip it's listed in the Notices centre with a way to open the workspace.
   - An **Update** goes in the Inbox. It isn't counted on the ✋, because a handoff isn't an ask.
   - The sidebar row keeps a "new" mark until the person has looked at the chat.

   This carries the safety property `crates/purlis-core/docs/handoff.md` gave the tab: a chat started by another chat always lands where the person can see it.

### Shells and short programs

7. **A shell follows the same rules as a chat.** New shell takes the focused pane. Replacing it or closing its pane leaves it running in the sidebar, so a dev server survives, and ending it is a separate action. When its program exits, its row goes and its pane clears. A pin works as a chat's does. The accepted cost: forgotten shells stay listed until they're ended.
8. **A one-job program opens beside, and goes when it exits.** A terminal editor (#1042) and a forge login typed into a shell open as a split beside the focused pane, in the same tab, with focus. When the program exits, that split goes and focus returns. No new tab. The rule: what the person asked to work in takes the focused pane, and one job for a moment opens beside.

## What this hands on

- **Prototype: one tab, the strip and its icons:** draws the empty pane, the sidebar row's "new" mark and the handoff's chip card, and what replaces today's three empty-window states.
- **Storing a tab's layout:** a tab needs its workspace stored, because an empty pane has no chat to derive it from. That departs from ADR 0036, which refuses to record a workspace on a tab, so the ticket decides it and the ADR gets an amendment.
- **Prototype: dragging, splitting and tiling panes:** this ticket settles presses only. Whether a sidebar drag can put a chat from another workspace into a pane is still open.
- **Rewriting the records:** `CONTEXT.md`'s Chat, Task, Handoff and Needs you entries, `handoff.md:115`, the shell tab entry and `docs/spec.md`'s terminal editor line.

No new tickets. Nothing was prototyped or built here, and the rules above were checked against the code by reading only.

Revised by [Storing a tab's layout](https://github.com/purlis/purlis/issues/4587#issuecomment-6571477964), as the person ruled on 2026-11-02. On the first launch after the upgrade, a workspace with no remembered front chat shows its first chat in the old strip order, pinned first, rather than an empty pane. An empty pane is only for a workspace with no chats.

Narrowed by [Prototype: dragging, splitting and tiling panes](https://github.com/purlis/purlis/issues/4583#issuecomment-6572372976), as the person ruled on 2026-11-02. "A tab shows only its own workspace's chats" now holds for presses only. A sidebar **drag** may put another workspace's chat into a pane. The chat stays filed in its own workspace, and the pane head names that workspace. A press still brings the other workspace forward.


## #4587
## Answer (the person, 2026-11-02: yes to all four)

Asked in one round through the asking chat for [A window with no tab per chat](https://github.com/purlis/purlis/issues/4588). D1 to D9 are the [charting decisions](https://github.com/purlis/purlis/issues/4576). Nothing was built or run, and the code was read at `origin/main` 9bae8bec3.

### Asked and ruled

1. **A tab records its workspace, and ADR 0036 gets an amendment.** Every tab records the strip it's on (a workspace, or `None` for the strip of chats outside every workspace), as view tabs already do: `reopen::View::workspace` says "A tab with no chat cannot be filed by where its chat works, so it says". A chat is still filed by the plane's answer, never by the tab it sits in, so a chat dragged into another workspace's tab doesn't move that tab. When a tab's workspace is removed, the tab closes and its chats stay in the sidebar. **The amendment's argument:** ADR 0036 refused a copy of the plane's answer, and a tab's strip isn't one. It's a fact the person made, which nothing else knows.
2. **The first launch after the upgrade.** The tab that was in front at quit comes back as it was, even if it's a plain one-chat tab. Every other workspace keeps its split, view and pinned tabs (D8). A workspace (or the plane root) that keeps none gets one tab showing its first chat in the old strip order, pinned first. That's what the old app showed on going back to a workspace after a relaunch (`showWorkspace`'s fallback, `tabs.ts:1077`), because its per-workspace "last in front" was only ever in memory (`lastFront`, `PlaneView.tsx:1318`). An empty pane only appears for a workspace with no chats at all. **From then on the record stores each strip's front tab**, which replaces that ref. This revises #4588's "otherwise an empty pane".
3. **After a harness chat's program exits**, its pane keeps the last screen, read-only, under an "ended" band with **Resume** and **Clear pane**. The chat's row stays in the sidebar. Shells, whose pane clears, and one-job programs, whose split goes, are as #4588 ruled.
4. **The core holds each tab's tree, and `looked_at` reads it.** The record, and `Chats` while the app runs, hold every tab as a tree, plus the front tab of each strip and the strip that's drawn. `looked_at` is every chat in a pane of the front tab on the drawn strip. Empty panes and views count as nothing, so a tab with no lead chat or with a cleared first pane is covered. `Chat.beside`, `Chat.shows` and `tab_shows` go. The core refuses a layout that names a chat twice, or a chat that isn't open, which guards D1's "never shown twice" in one place. A press sends its new layout in the same call, so a task the person presses onto is held from that moment and can't be ended under them (`is_held_back`, `close_refused`). After a launch or reload, the stored layout stands until the window reports, and it's what the window draws from. Notifications are also gated on the window being in front (`asknotify.rs`), so a stale layout with the window in the background can't swallow one.

### Decided without asking (minor)

- **The record's shape:** a new `tabs` list in `reopen.json`, in strip order, one entry per tab. Each holds `workspace`, `pinned`, `front` (this strip's front tab), the focused pane as a path from the root, and the tree:
  - a node is a **pane** or a **split**;
  - a split holds `direction` (`row` or `column`), `share` (the first child's share in tenths of a percent, 0 to 1000) and exactly two children, so it stays binary as D6 and `LayoutPanes` keep it;
  - a pane holds a **chat** (its number), a **view** (today's `View` fields, without the tab-level ones), or is **empty**.
- **Sizes are per tab.** Nothing carries between tabs, so `LayoutPanes` gets keyed by tab. A new split starts at 50/50, and "Tile all" writes even shares, so three panes in a row are 333/667 then 500.
- **An empty pane** is a third pane kind, drawn with New chat as #4588 rule 1 says. Closing a pane in a split collapses the split. Closing a tab's only pane closes the tab, unless it's its strip's last tab, which keeps one empty pane.
- **One-job splits are never written to the record.** A launch has no program for them.
- **Pane numbers stay launch-local.** A pane is named in the record by its path.
- **A tab's pin is recorded.** What a pin means now is the map's Pinning fog.
- **Upgrade from today's record:** chats sharing a `beside` become one tab in a flat row with even shares, ordered by the strip. A pinned chat or view becomes a pinned tab. A view that was beside a chat was never recorded, so it's lost. A record with no `tabs` list is read the old way exactly once.

### What this hands on

- **The spec** gets one core-owned item: the tab tree in `reopen.rs` and `Chats`, with `looked_at` derived from it and the window reporting layouts. Its tests must pin the two silent failures: a task ended under the person, and a notification lost.
- **Rewriting the records:** an amendment to ADR 0036 (a tab records its strip). ADR 0043's "a tab is a layout of panes" gains the empty pane.
- **[Prototype: one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582)** draws the ended band (Resume, Clear pane) and the empty pane.
- **[Prototype: dragging, splitting and tiling panes](https://github.com/purlis/purlis/issues/4583)** can rely on stored shares and a before or after side for each split.

No new tickets.


## #4585
## Answer (the person, 2026-11-02: accepted, all four as recommended)

Prototype: throwaway branch [`prototype/pane-chat-actions`](https://github.com/purlis/purlis/tree/prototype/pane-chat-actions) (commit 1bf2329b4), never merged. The folder is `app/src/prototype-pane-actions/`: a one-file page with variants A/B/C and six scenes, a README, and **the final command table, [`command-table.md`](https://github.com/purlis/purlis/blob/prototype/pane-chat-actions/app/src/prototype-pane-actions/command-table.md)**. There are no screenshots yet because machine load was high. They can be added later.

1. **The pane head: one state button, then ⋯ (A).** Left to right: icon, name, state mark, "harness · workspace", then the state button, ⋯, the bell slot (#4584), Split right, Split down and Close pane. The state button is Stop while the chat works, Answer while a task asks, Cancel smart close while it wraps up, Brief on a quiet task, and nothing on a quiet chat. Everything else is in ⋯. A narrow pane drops parts in a fixed order: "harness · workspace" under 520px, then the mark's word, the button's word, the mark, and Split down under 300px. The bell, ⋯, Split right and Close never go. The skills and harness chips leave the head and become rows of the list. Ruled out: the name as the menu (B), because Stop takes two presses and the name doesn't look pressable; and a folding toolbar (C), because it's busy and its buttons shift on resize.
2. **One list per chat.** The pane head's ⋯, the sidebar row's ⋯ and right-click, and the palette read one ordered list, keyed by the chat and not the tab. The palette's word is the menu's word with the chat's name put in. Order: rows for the state now, then the chat, open, run, branch, end. A row that can't apply to this kind of chat is left out. A row that applies but can't run now is shown greyed with its reason. A test holds the three surfaces equal.
3. **Shells** (#4588) keep Rename, Link, Open in new tab, Activity, Network, Stop, the branch rows and End. They have no Use skill, Ask, Skills, Harness, Restart, Start fresh, Move to, Stop and everything below it, Stop all tasks, Smart close or task rows. End… on a shell asks only Cancel or End.
4. **A tab's right-click holds tab things only:** Pin tab, Tile all, Keep open (a preview tab), Close other tabs, Close tab. Closing never ends a chat, and "Close and end the chat beside it" goes. Because every chat row leaves the tab, a split tab can no longer act on its first pane's chat (#4577). Right-click on a pane head shows the chat's list, a line, then the pane's rows (split, Tile all, Copy, Paste, Send F2, Close pane).
5. **Smart close (ADR 0064)** stays the third answer of End…'s dialog, reached wherever End… is. **Its strip chip goes** (`ChatStrip.tsx` `isBackground`, `StripMarks`), because a chat has no tab. Wrapping up is shown as a breathing mark on the pane head and the sidebar row, with Cancel smart close as the head's state button and first in the list. Once the record is written the chat ends like End: the pane keeps its last screen with Resume and Clear pane (#4587) and links the record. A stop without a record goes to the pane's band, the bell and the needs-you row, as today, with Dismiss on the list.
6. **A sidebar row shows ⋯ on hover and focus** with the same list. Right-click and Shift+F10 stay.

**Gone or moved, for the spec:** 31 Move to its own tab becomes Open in new tab, for any chat. 33 Send back goes, because Close pane does it. 34 Switch to leaves the tab menu, since the tab's press does it. Of #4577's 19 one-surface rows, none is left among chat, pane and tab rows. 38–41 (This tab's tasks, Next/Previous chat in this tab, Back to this tab's own chat) go to the map's "Keyboard" fog, and Move to new window to "Several windows".


## #4582
## Answer (the person, 2026-11-02: accepted, all four as recommended)

Prototype: throwaway branch [`prototype/one-tab-strip`](https://github.com/purlis/purlis/tree/prototype/one-tab-strip) (commit c51e90dc8), never merged. The folder is `app/src/prototype-one-tab/`, with a README, a one-file page and screenshots of 8 scenes × 3 strip treatments. Private page: https://claude.ai/artifact/MhudbcYecnuNedfhhGEicU. It draws 24 chats in two workspaces and the plane root. Every row, tab, pane head and card wears the real `ChatPersonaState`.

1. **The strip: Name tabs (A).** The strip is always shown, even with one tab. A tab shows its focused pane's combined persona-and-state icon and the chat's name. A split adds `⊟+N`, and hovering the tab lists every pane. An empty pane's tab says "New chat" with a plus, and a view's tab shows its own icon and name. A pinned tab has a pin before its icon. Ruled out: hiding the strip at one tab (B), because the strip would pop in at the first view or Open in new tab; and layout tabs drawn as one icon per pane (C), because icon-only tabs are hard to tell apart.
2. **One empty pane replaces the three empty windows** (`PlaneView.tsx` empty-workspace, empty-plane and empty-window). It reads "Nothing in this pane" with how many chats are running in the workspace, "pick one in the sidebar", and **New chat**. A plane with no workspace says "No workspaces yet" and adds **Create a workspace**. An empty pane in a split has its own ×.
3. **The sidebar marks what's on screen.** The focused pane's chat has the highlighted row. A chat shown in another pane of the same tab has a faint bar at its row's edge, so a press that will jump (D1) can be seen first. A handoff that arrived has a **new** badge until the chat is looked at. A row above a chat that needs you shows ✋. Workspace and All stay, and This tab goes (D5).
4. **The handoff card** hangs from the target workspace's chip: "<chat> arrived from <asking chat>", with Go to it and dismiss. A press anywhere else hides it and leaves a dot on the chip, and pressing the dot brings it back. It takes no pane and no focus.
5. **The sidebar pick isn't measured on the real path.** The prototype's press to drawn frame is about 29 ms median over 20 picks, but that's only its React work, with no terminal. The real path (terminal rebuild plus 5,000 lines of scrollback) went to [Measure the sidebar pick on a built app](https://github.com/purlis/purlis/issues/4692).

**Found while building, for the spec:** the combined icon's badge-on-the-corner CSS is scoped to the sidebar (`App.css:3382`, `.chats-section .chat-persona`). Outside it the state mark draws beside the persona, not on it. Reusing it on tabs, pane heads and cards means lifting those rules out of `.chats-section`, with each surface giving the badge's ring its own ground (`--chat-badge-ground`).

**Not settled here:** keyboard, narrow windows and what the strip does when it overflows stay under the map's "Not yet specified". Pane chrome beyond the head drawn here is #4583's and #4585's.


## #4584
## Answer (the person, 2026-11-02: variant A, recommendation on all four, two fixes)

Prototype: branch [`prototype/pane-notices`](https://github.com/purlis/purlis/tree/prototype/pane-notices), `app/src/prototype-pane-notices/` (throwaway, never merged). Open it with the command in its `README-PROTOTYPE.md`. Accepted state: `screenshots/A-s4-min.png` and `screenshots/A-s4-exp.png`, commit 2d318cf15.

1. **Where the bell sits:** in the pane header, in a fixed slot just before split and close. The slot is there even with no count, so nothing beside it moves.
2. **The band sits under the pane header** (the person's fix), so the header never moves when notices arrive. The order is: the task-question strip (#4561), then the header, then the band, then the terminal pinned to its bottom. The band takes about half the pane at most and scrolls inside past that.
3. **A consent or blocking ask stays open when minimised as its full card, unchanged.** It has no ✕. A one-line form was rejected because it cut a consent's words: the host name was lost in a narrow pane.
4. **Dismiss all** sits on the band's head row when expanded, beside Minimise: "7 notices, 2 waiting on you · Dismiss all 5 · Minimise". It skips the asks, and its label counts what it clears.
5. **What the count says:** the notes the pane holds, the same number minimised and expanded (the person's fix: it read 5 minimised and 7 expanded). Asks are never counted, because they're always open. The count is neutral, and the number takes the trouble colour when one note is trouble. Expanded shows as the bell pressed (`aria-expanded`), not as a different number. The screen reader hears "5 notices, 1 in trouble, 2 asking". This matches #4551's "1 asking, no red".

Held in every variant and kept: nothing draws over the terminal (no popover); a new notice never opens the band or takes focus, and its count is announced politely; Esc on a focused note dismisses it; one minimised or expanded setting covers every pane (D7).

**Consequences for the build** (decided here, delegated): expanded shows every notice, so the pane's cap of two (`MOST_ON_A_PANE`, `inboxRules.ts`) and its "N more wait in the Inbox" line go. The minimised or expanded choice is one app preference, minimised by default, and kept across restarts. A paneless chat's asks follow #4588 (row, ✋, chip, Inbox). The build must check WebKit and real terminal reflow, which this page, drawn in Chromium, did not.

No new tickets.


## #4583
## Answer (the person, 2026-11-02: accepted, variant A, the recommendation on all four)

Prototype: throwaway branch [`prototype/pane-drag`](https://github.com/purlis/purlis/tree/prototype/pane-drag) (commit 86cdae0ef), never merged. It was cut from `prototype/one-tab-strip`, so it wears #4582's accepted strip A. The folder is `app/src/prototype-pane-drag/`, with a README holding the `open` command and one self-contained page: 6 scenes × 3 variants. Each tab is a binary tree with shares in tenths of a percent, as [Storing a tab's layout](https://github.com/purlis/purlis/issues/4587) ruled. Icons are the real `ChatPersonaState`, controls the real `app/src/ui`. No screenshots: load was high, and the sandbox can't launch Chromium. Every scene and variant was rendered, and each drop checked, in jsdom.

### Asked and ruled

1. **Variant A: zones plus placement mode.** While a pointer drags, the pointer's place in a pane picks the move. The outer quarter by an edge splits on that side, and the middle swaps. The result is drawn on the pane before the drop, with a sentence ("reviewer 1 right of steward 4."), and a refused place is drawn dashed with its reason. **Keys and single presses** use placement mode. Pressing a pane's grip (its icon and name), Enter or Space on it, or ⌘⇧M on the focused pane, picks the pane up. Every other pane then shows five targets (above, left, swap, right, below) as real buttons, and the strip shows a New tab target. Arrows move inside a pane's targets, Tab goes to the next pane, Enter drops, and Esc puts it back and returns focus. This is the single-pointer alternative WCAG 2.5.7 needs. Ruled out: B, targets only for the pointer too, because every drag aims at a 34px button; and C, a list of places for keys, which loses the panes' positions at 5+ panes.
2. **Another workspace's chat may land in a pane, by drag only.** A drag is deliberate. The chat stays filed in its own workspace (#4587 rule 1), and the pane head names that workspace with a badge. The drop's sentence says "it stays filed in volaticloud". **A press** still brings the other workspace forward, as [A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) ruled, which narrows that ticket's "a tab shows only its own workspace's chats" to presses.
3. **These moves are added to D6's list:**
   - **Zoom a pane:** it fills the tab for now (⌘⇧Enter). The strip's tab shows a zoom mark.
   - **Undo the last layout change:** a toast with Undo after every drop, close, split and Tile all, and ⌘Z.
   - **Drag a tab into a split:** a tab dragged down off the strip splits in beside a pane with all its panes. A tab of several panes can't go into a pane's middle.
   - **Hold over a tab to bring it forward:** after half a second, so a pane can land in another tab's layout.
   - **Even a split:** double-click its separator, or press Enter on it. Arrow keys resize it by 5%.
   - **Esc cancels a drag.**
   - **A minimum pane size:** about 40 columns by 8 rows (300 × 140 px in the prototype). A split that would leave a half under it is refused, and says so.
4. **Tile all tiles the tab's own panes**, in reading order, with Undo. It never pulls in a chat nobody put there (D1, D4). The grid has ⌈√n⌉ columns, filled row by row, with even shares: 6 panes make 3 × 2 at 333 then 500. One-job panes are tiled like any other. It lives at the strip's right end (⌘⇧G), and is disabled at one pane.

### Decided without asking (minor, drawn in the prototype)

- **What each drop does.** On an edge, the source leaves its place, its sibling takes the split, and the source is inserted as the first child for left and top, the second for right and bottom, at 500. In the middle, pane on pane swaps. A sidebar chat replaces what the pane shows, and the replaced chat keeps running in the sidebar (D1). Onto an empty pane, the source fills it and its old pane closes. On the strip, a line shows where the new tab goes.
- **A chat already on screen acts as its pane** when dragged from the sidebar, so it moves rather than showing twice (D1).
- **A pane's last chat leaving its tab** closes that tab, unless it's the strip's last tab, which keeps an empty pane (#4587).
- **What can be picked up:** a chat, a view, a sidebar row or a tab. An empty pane has no grip; close it instead. A **one-job pane** has no grip, no close and no swap, because it goes when its program exits (#4588). Its edges still take a split.
- **Split's direction** follows the pane's shape: beside when it is wider than tall, below otherwise.
- **Sidebar rows:** the row drags; its press stays a press. A **Place…** button on the row (and ⌘⇧M) is the keyboard route.
- **What is announced,** politely: "Picked up developer 7. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.", then each target's sentence as focus moves, then "Dropped. <sentence>", "Not dropped: <reason>" or "Put back. Nothing moved."

### Found while building, for the spec

- `tabs.ts` can only add a pane as the second child (#4581), so left and top drops need an insert-before.
- `LayoutPanes`' drop is on the whole `PaneFrame` and reads no pointer position. It needs the edge and centre hit test and an overlay above the terminal, because xterm eats pointer events.
- The strip's tab drag is locked to its row (`ALONG_THE_STRIP`, `sortable.tsx`). Taking a tab into a split means lifting that lock past a vertical threshold.
- The minimum size belongs in `react-resizable-panels`' `minSize` too, or a separator drag goes under the floor the drops keep.
- Zoom and Undo are per tab. Undo holds the tab's last tree in memory only.

### Not settled here

The shortcut keys (⌘⇧M, ⌘⇧G, ⌘⇧Enter) are placeholders for the map's **Keyboard** fog. A drag between windows stays under **Several windows**, and narrow windows under **Narrow windows**. The build must check WebKit, real terminal reflow and a drag across a live xterm, none of which this Chromium/jsdom page did.

No new tickets.


## #4699
Addendum (orchestrator, 2026-11-02, delegated call): [Can an extension's view ask for a pane?](https://github.com/purlis/purlis/issues/4703) handed this ticket a key for **Open beside**, and it arrived after the resolution. Ruling: Open beside gets no chord of its own. Holding ⌥ (Alt off a Mac) while pressing a view's button, or pressing ⌥Enter on a palette row, opens it beside, which is the VS Code convention. Plain Enter and a plain press keep opening a tab. [Prototype: an extension view's Open beside and its offer notice](https://github.com/purlis/purlis/issues/4705) draws it.


## #4701
## Answer (the person, 2026-11-02: the recommendation on all four)

Asked in one round through the asking chat for [A window with no tab per chat](https://github.com/purlis/purlis/issues/4588). D1 to D9 are the [charting decisions](https://github.com/purlis/purlis/issues/4576). Nothing was built or run. The code was read at `origin/main` 6b8cee297.

### What several windows can do today

**A project can't have two windows.** A window holds whole projects, and each project is held by one window (ADR 0033, amended 2026-09-26). `Showing` in `planes.rs` keeps that, and `windows.rs` acts on it. `move_projects` moves whole projects only, and refuses a project that another window holds. A press or a Go on a chat in another window's project calls `show_window_holding`, which brings that window forward and carries the press out there (`windows.ts` `runElsewhere`). It never opens the project twice. Each project has its own `Chats`, so `in_front` and `looked_at` are already per project, and so per window. The only "Move to a new window" row today is the project tab's (`menuModel.ts` `project.window`). A chat tab has none.

### Asked and ruled

1. **A tab window.** One project's chats can be on screen in two windows: a tab can be torn off into a **tab window** of the same project. A tab window has the strip and the panes, but no sidebar and no project strip. ADR 0033 still holds: the project is held by one window, its **project window**, and its tab windows belong to it. Closing a tab window sends its tabs back to the project window, behind its front tab, and every chat keeps running (D2). Its last tab leaving closes it, as a split window that holds nothing goes today. When the project moves to another window, its tab windows stay its own. Closing the project closes them too. This is VS Code's floating-window model.
2. **What a press acts on.** If the chat is on screen in any of the project's windows, that window comes forward and focuses the chat's pane (D1). Otherwise the chat goes to the **most recently used pane across the project's windows**, which is #4588's rule 2 stretched across windows. A view is never replaced. The palette in a tab window acts on that window's focused pane.
3. **Moving tabs and panes between windows.** A tab's **Move to new window** makes a tab window, and so does dragging a tab off the edge of its window. A tab and a pane both get **Move to window ▸ <window>**, which lands it as a new tab there. The rows are in the menus and the palette, so no move lives on one surface only (D9). A chat never leaves its project's windows. A drop into another window's edge or centre zones is left to [Dropping a pane into another window's zones](https://github.com/purlis/purlis/issues/4706), because a webview can't carry an HTML drag into another one.
4. **Tab windows come back.** Each tab's record in `reopen.json` ([Storing a tab's layout](https://github.com/purlis/purlis/issues/4587)) says which window it was in: the project window, or tab window *n* with its own front tab. A launch makes those tab windows again. Where each window sits on screen stays machine state (ADR 0034's "how they were arranged in windows"), keyed by the project and the tab window's number.

### Decided without asking (minor)

- **`looked_at` per window:** every chat in a pane of the front tab of each of the project's windows that is on screen. That is the drawn strip's front tab in the project window, and the front tab of each tab window. A minimised or hidden window counts as nothing. #4587 rule 4 still holds otherwise: the core holds the trees, a press sends its layout in the same call, and the core refuses a layout that names a chat twice **across all of the project's windows**, which keeps D1 in one place.
- **`in_front`** becomes one front tab per window: the project window's front tab per strip, as #4587 ruled, and one front tab for each tab window.
- **Notifications** still need the window showing the chat to have the keyboard (`asknotify.rs` `Looking.window_in_front`), asked of the window that shows the chat.
- **A tab window has one strip.** Its tabs can come from any workspace of the project, and each tab keeps its workspace (#4587 rule 1) and shows that workspace's name. A press in a tab window on a chat of a different workspace than the focused tab's follows #4588 rule 3: it brings that workspace forward in the project window.
- **A project's events** go to the project window and to each of its tab windows. `emit_for_plane` sends to one holder today, so it needs to send to the set.
- **Labels:** a tab window is `window-<n>` like a split window, so the capabilities need nothing new. `Showing` records which project each tab window belongs to.

### What this hands on

- **The spec:** tab windows. That means `Showing` holding a project window plus tab windows, `emit_for_plane` sending to the set, the tab record's `window` field, and `looked_at` and the no-twice check across windows. Its tests must pin a press on a chat shown in a tab window bringing that window forward, and a task in an on-screen tab window not being ended.
- **[Rewriting the records](https://github.com/purlis/purlis/issues/4704):** an amendment to ADR 0033 (tab windows), and a note on ADR 0034 (the place on screen of each tab window).
- **New ticket:** [Dropping a pane into another window's zones](https://github.com/purlis/purlis/issues/4706) (research).


## #4700, #4703, #4705, #4702 (as record notes on #4704)
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Pinning a tab now that a chat isn't a tab](https://github.com/purlis/purlis/issues/4700): **ADR 0039 amendment**: pinning is at three levels, project, workspace and **tab**. The chat pin retires, and a tab's pin keeps the tab (its place first on the strip, and the tab across relaunch, upgrade and cleared panes) but not its contents. The pin moves from `Chat::pinned` to `reopen.json`'s `tabs[].pinned`. Also reword `reopen.rs`'s `pinned` doc ("Whether the operator pinned this chat") and `tabs.ts`'s `Pinned` doc to say tab.

--
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Can an extension's view ask for a pane?](https://github.com/purlis/purlis/issues/4703#issuecomment-6572628156) (2026-11-02):

- **ADR 0043, amendment:** an extension's view never says where it goes. A press opens it in a tab of its own, or with **Open beside** in a split beside the pane pressed from (from the palette, the focused pane), and it never replaces a pane's content. Without a press an extension can only offer its view, with one notice that takes no pane, tab or focus. The manifest gains no key, and this answers the view-tab amendment's open question "whether a view may be split beside a chat by anything but starting a chat beside it".

--
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Several windows: chats, panes and tabs across windows](https://github.com/purlis/purlis/issues/4701#issuecomment-6572745426), add:
- ADR 0033: a project may have **tab windows**, which hold torn-off tabs (strip and panes, no sidebar) and belong to the one project window that still holds the project. Closing one sends its tabs back to the project window.
- ADR 0034: what is arranged in windows now includes each tab window's place on screen, keyed by the project and the tab window's number. Which tab is in which window goes in the plane's `reopen.json`.

--
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Keyboard and focus once a chat isn't a tab](https://github.com/purlis/purlis/issues/4699): **`docs/ui-primitives.md`** (the task-keys table at about line 2855, its xterm.js note, and the Focus mode line at about 2558) and **`taskKeys.ts`'s header**: the tab-task keys become ⌘⇧H "Back to the chat this pane showed before" and ⌘⇧J "The focused chat's tasks"; next and previous chat go; ⌘⌥ plus arrows moves between panes; ⌘⇧] and ⌘⇧[ (Ctrl+Shift+] and [) move between tabs; ⌘⇧Enter is "Focus this pane"; ⌘⇧M picks up a pane. `sideKeys.ts`'s ⌘⇧J note says "the focused chat's tasks".

--
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Prototype: the strip and panes in a narrow window](https://github.com/purlis/purlis/issues/4702) (2026-11-02), add to **ADR 0039's amendment**:

- **Its last open question is settled. A pinned tab is exempt from the collapse.** Pins are always drawn, and go icon-only when they alone would take more than half the strip. Tabs share the row down to a **120px floor** (icon, about eight letters, `⊟+N`) before going behind show-more. The 2026-09-22 amendment's sentence "a pin draws a tab first, so it is the last thing to go, and no exemption of its own" is replaced.
- **ADR 0043:** a split that can't give both sides about 40 × 8 folds the side without the focus to a rail or bar when drawn. The stored tree and shares don't change. Side regions never give way by themselves.

--
author:	diazoxide
association:	member
edited:	false
status:	none
--
From [Prototype: an extension view's Open beside and its offer notice](https://github.com/purlis/purlis/issues/4705) (accepted 2026-11-02): ADR 0043's view-tab amendment (the #4703 line) also records that an offer is a pane-band and Notices-centre note, that Dismiss keeps that view and subject quiet until relaunch, that "Stop offers from <extension>" is a per-extension preference in Settings › Extensions (no new ADR 0041 capability), and that a removed extension's offers are withdrawn while its open view shows "not here any more".

--
