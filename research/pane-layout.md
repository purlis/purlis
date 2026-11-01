# What the pane layout supports today

Research for purlis/purlis #4581, under map #4575 and the charting decisions in #4576.

Primary source: the code at `origin/main` 7e723d5d4 (2026-11-01), read only. Nothing was run, so
anything about how the running window behaves is read from code, and section 9 lists what that
leaves open. The one source outside the repo is `react-resizable-panels` 4.15.0, the pinned version
(`app/package.json:69` says `^4.15.0`, the lockfile resolves 4.15.0). I read its shipped
`react-resizable-panels.d.ts`, cited below as `rrp.d.ts`.

Paths are from the repo root. `tabs.ts`, `LayoutPanes.tsx`, `PlaneView.tsx` and the like are in
`app/src/`.

## The short answer

- **Nesting is unlimited, but the tree is binary.** Each split has exactly two children and no size.
- **Pane sizes are stored nowhere.** Neither the tab state nor the record holds them.
- **The tree isn't stored either.** The record keeps only "this chat is beside that one", so a
  relaunch rebuilds splits only for tasks. Every other chat comes back as its own tab.
- **Panes have no minimum size.** They use the library's default of 0%.
- **The only drop on a pane is a file or text reference.** Nothing can drag a pane or a chat.
- **A chat that's closed loses its pane; a program that exits by itself keeps it.** Clearing a pane
  isn't a state the model has.
- **One session per pane holds; one session per tab doesn't.** The real assumption is that each tab
  has a lead chat in its first pane, and that each pane has an "own" chat that's the anchor for
  tasks. Decisions 1 to 4 and 8 of #4576 cut across both.

## 1. How deep splits nest

- `Layout` is recursive: a pane, or a split of `direction` with `children: [Layout, Layout]`
  (`tabs.ts:288-290`). `Direction` is `row` or `column` (`tabs.ts:52`).
- There's no depth limit. `LayoutPanes` recurses through its children (`LayoutPanes.tsx:777-847`),
  and `panes`, `replace` and `without` walk the tree (`tabs.ts:1781-1805`). I found no cap in them.
- **Always binary.** `LayoutPanes.tsx:794` says so: "A split has exactly two children and they never
  swap, so the side IS the identity." Three panes in a row are nested splits, not one three-child
  split.
- Every way of making a split wraps one pane: the focused pane in `splitFocusedPane`
  (`tabs.ts:1114-1138`), or a named pane in `placeBeside` (`:1642`), `openViewBeside` (`:1696`) and
  `openViewBesideView` (`:1730`). The last three are always `row`. The new pane is always the second
  child: nothing can insert before a pane, so a left or top edge drop has no function to call yet.
- Closing collapses the split: `without` hands the surviving side up (`tabs.ts:1799-1805`).

## 2. How sizes are stored and restored

**They aren't.** The sizes live only in the mounted `react-resizable-panels` `Group`.

- The nested `Group` and `Panel` are drawn with no `id`, `defaultSize`, `defaultLayout`,
  `minSize` or `onLayoutChanged` (`LayoutPanes.tsx:778-798`). The window's regions do all of this
  (`RegionFrame.tsx:480-502`, `:798-802`, `:1161-1165`) and write to `layout.json`; panes don't.
- `Layout` has no size field (`tabs.ts:288-290`).
- `Tabs.splits` is **not** pane sizes. It's a view's internal divider, such as a file tab's tree
  beside its preview (`tabs.ts:349-354`, `setSplit` at `:363`), and it's what `reopen::View::split`
  keeps (`crates/purlis-core/src/reopen.rs:564-567`).
- `layoutPref.ts` isn't involved. It's the store for one person's preferences in `layout.json`
  (`layoutPref.ts:7-20`), such as the editor or the Chats list. It's a fit for decision 7's remembered
  minimised-or-expanded choice, not for per-tab geometry, which belongs with the tabs.

**What the record holds** (`.charter/app/reopen.json`, `reopen.rs:581-592`):

| Fact | Where | Restored? |
| --- | --- | --- |
| Chat pinned, label, order | `Chat` fields, `chat_order` (`app/src-tauri/src/lib.rs`, after `:2680`) | yes |
| A pane's shown task | `Chat.shows` (`reopen.rs:188`) | yes, if still below that chat (`PlaneView.tsx:1689-1694`) |
| Which chat's tab it's in | `Chat.beside` (`reopen.rs:200`) | **only for tasks** (`PlaneView.tsx:1679-1684`, `reopen.rs:193-195`) |
| View tab | `View`, first pane only (`reopen.rs:544-567`) | yes, as a tab |
| View beside a chat | nothing | **lost** (`PlaneView.tsx:10951-10955`) |
| Tree shape, direction, sizes, focused pane, `tabs.order` as such | nothing | **lost** |

`viewTabsOf`'s own comment says it: "a chat's split is not recorded either, and both come back as
what they are: the chat as its own tab, the view as nothing" (`PlaneView.tsx:10951-10955`).
`renameTab` says the same (`tabs.ts:405-406`). Restored task splits are always rebuilt by
`placeBeside`, so they're always `row`, even if they were `column` before.

`beside` is flat: it says "in the tab led by chat N" and nothing else. `panesSaid` writes it for every
non-lead chat of a tab (`tabs.ts:678-689`), so a tab with three chats records two entries with the
same `beside`.

**Inside one launch** there's a further gap. `LayoutPanes` has no `key` at the call site
(`PlaneView.tsx:9865-9870`), and only the front tab's panes are drawn (`tabs.ts:25-26`). So by
React's rules, switching between two tabs with the same top-level shape should reuse the same
`Group` and carry its sizes across, and switching through a tab of another shape should throw them
away and start even. That's inferred, not seen (section 9).

## 3. Minimum sizes

**There are none for panes.** `<Panel>` passes no `minSize` (`LayoutPanes.tsx:798`), and the library
default is 0% (`rrp.d.ts:857`, "defaults to 0%"). The CSS doesn't add one: `.pane-frame` is
`min-height: 0` (`App.css:718-724`). So a separator can be dragged until a pane has no room, and I
found no floor in code on how small a terminal gets.

The only size awareness in a pane is `useRoomy`: under 40rem the Skills chip is dropped
(`LayoutPanes.tsx:286-307`, `:238`). Regions do have floors (`RegionFrame.tsx:481`, `:490`). #4575
lists each pane's minimum as not yet specified, and "Tile all" makes it urgent: at some chat count
an even grid can't fit.

## 4. What drag exists

Four mechanisms, none of which moves a pane:

| Mechanism | Where | What it does |
| --- | --- | --- |
| Native HTML drag | `references.tsx:57-83`, `Explorer.tsx:836`, `:2083` | A file, folder, lines or search hit dropped on a chat. |
| `@dnd-kit` | `ChatStrip.tsx:178-195`, `sortable.tsx` | Reorders tabs along the strip, with keyboard and screen reader words. |
| `@dnd-kit` | `ProjectRail.tsx:282` | Reorders projects. |
| Separator | `ui/panel/Separator.tsx` | Resizes a split, by pointer or arrow keys. |

`onDragOver` in `LayoutPanes` (`:194`) is the reference drop, and nothing else:

- `overChat` accepts only `REFERENCE_TYPE` (`references.tsx:71-75`).
- `onDrop` types the reference into the chat, unsent (`LayoutPanes.tsx:195-200`).
- It's on the whole `PaneFrame`, so the pane has no notion of edge or centre. It reads no pointer
  position.
- A view's pane (`:551`) and a task-away pane (`:654`) have no drop at all. An ended-task pane and a
  view can't take a reference.

The strip behaves the same way. Its `onDragOver` takes references (`ChatStrip.tsx:282-291`), and its
tab drag is locked to it: `ALONG_THE_STRIP` restricts to the horizontal axis and the parent
(`sortable.tsx:56`), and `sortable.tsx`'s header says outright that a tab can't leave its row.

## 5. What happens to a layout when its chat ends

`closeChat` is the one function for "the core already closed this chat" (`tabs.ts:1165-1186`): the
pane goes, the sibling takes the split's place, and a tab's last pane closes the tab. It's called
when:

- a smart close lands (`PlaneView.tsx:2178`);
- the person stopped the chat (`:2211`);
- Close on a kept-open Notice (`:2439`);
- a restored chat didn't come back (`:6400`);
- stopping several at once (`:8228`).

Two variations:

- **A task shown in another chat's pane stays**, drawn ended with the way back. `closeChat` only
  looks at panes' own chats (`tabs.ts:1175-1178`), and `shownLive` turns the pane non-live
  (`tabs.ts:1321-1332`), which `LayoutPanes` draws as `TaskAway` (`LayoutPanes.tsx:644-706`).
- **A chat started again keeps its pane.** `replaceSession` swaps the number in place
  (`tabs.ts:1211-1232`, `PlaneView.tsx:5825`, `:6611`).

**A program that exits by itself is different: nothing leaves the layout.** "The chat stays, as its
tab does" (`a_program_ended`, `app/src-tauri/src/chats.rs:6445-6449`). The core marks the session
ended and drops what it owed (`its_program_ended`, `chats.rs:3340-3354`), and the pane stays. One
exception is a resume that failed before the harness reported, where the same record starts again
as a fresh chat (`lostOnResume`, `sessions.ts:65`).

**The close control ends the chat today**, which decision 2 reverses. `closePane` runs
`closeFocusedPane` and then `commands.closeSession` (`PlaneView.tsx:3638-3654`); `close` does the
same for the whole tab (`:3665-3682`). A task's pane is the exception, because it's sent back and
ends nothing (`tabs.ts:1622-1630`).

## 6. What's missing

### For the drops (decision 6)

- **A drop model.** No function moves an existing pane. The nearest are `moveToOwnTab` and
  `placeBeside`, and both are task-only, driven by lineage (`tabs.ts:1599`, `:1642`).
- **Before-side insertion.** `children: [focused, new]` is hardcoded, so left and top edge drops
  need a side argument (section 1).
- **Split any pane, not just the focused one, with an existing chat.** `splitFocusedPane` is tied to
  the focus and is only called for a just-started session (`PlaneView.tsx:3509`). `Content` holds the
  chat's name beside its session, so a moved chat has to carry its name.
- **A centre swap.** It can exchange `content` between two panes without touching the tree, which
  respects the "they never swap" hazard in `LayoutPanes.tsx:780-794`. `Tab.shows` must move with it
  or go first.
- **Live-tree safety.** That comment records a crash from changing a `Panel`'s identity inside a live
  `Group`. Any drop that restructures the tree has to keep to "nothing is added to or removed from a
  live group", and the ids and keys need settling in the spec.
- **A terminal move costs a reattach.** `SessionPane`'s effect depends on `[plane, session]` and
  disposes the terminal when either changes (`SessionPane.tsx:408-429`). The core keeps the history
  and it re-reads it (`:396-402`), so nothing's lost, but a swap rebuilds two terminals. A pane that
  moves position in the tree remounts too, because `LayoutPanes` panes have no key.
- **A drag source.** There's nothing to pick up: pane headers aren't draggable, and neither are Chats
  rows (only `Explorer.tsx` is, among the sidebars).
- **Drop zones.** `PaneFrame` needs edge and centre hit-testing and a drop preview. The terminal sits
  under it and takes pointer events, and a drag has to work across it.
- **One drag system across surfaces.** The strip's `@dnd-kit` is locked to the strip, the panes use
  native drag, and decision 6 needs one drag spanning sidebar, strip and panes. It also has to carry
  on telling chats apart from references, which are dropped on the same pane.
- **A keyboard route.** The strip has one (Shift+Space, `sortable.tsx`). #4575 asks for one for every
  drag. Nothing exists for panes.
- **Dropping on the strip.** That needs a tab made from an existing chat, with the source pane
  cleared or removed. `moveToOwnTab` is the nearest shape, but it's for tasks.

### For "Tile all"

- **Sizes in the tree.** This is the main one. A binary tree with even default sizes can't draw an
  even grid unless the count is a power of two: three panes in a row nest as 50/25/25. So `split`
  needs a stored share, passed to `Panel` as `defaultSize`, or an n-ary split. `rrp.d.ts` offers
  `defaultLayout` (`:531`) and a `Grid` (`:127`), but decision 6 keeps the tree of rows and columns,
  so a share on `split` is the smaller change.
- **A build function.** Nothing builds a balanced tree from a list of panes. It's a pure function and
  easy to test.
- **A cap.** Without a pane minimum, or a limit on how many chats tile, 20 chats give 20 unusable
  panes.
- **Which chats.** "All" isn't defined: all in the tab, the workspace, or everything running? Only
  the tab's panes are obvious, and those are the ones that can be drawn. The spec needs to say.
- **`Group` re-registering.** `RegionFrame.tsx:396-397` records that a `defaultSize` that changes
  re-registers the panel, and ties that to charter-app#141. So applying a tile to a live group needs
  the library's imperative resize, or a remount, not a changed prop.

### For the other decisions

- **Clearing a pane (decision 2).** `Content` is `session` or `view` (`tabs.ts:251-286`), so there's
  no empty pane, and nothing reads as one. The likely third kind touches every `content.kind` switch.
  Whether a cleared pane stays or collapses its split isn't decided in #4576 either, and neither is
  what happens to a tab's only pane.
- **Picking a chat (decision 1).** `showChat` opens a new tab when the chat has no home
  (`PlaneView.tsx:3810-3815`), and `switchTabTo` moves the focus to the chat's home instead of
  replacing the focused pane (`tabs.ts:1357-1365`). "Jump, never show twice" exists for a chat with a
  pane, but it's found by the first match, not guarded.
- **Chats with no pane (decisions 3, 4).** Only a task chat or a grade's chat may have no tab today
  (`Chat::has_tab`, `reopen.rs:280-287`; arrival checks at `PlaneView.tsx:1850`, `:6386`). A handoff
  still opens a tab behind (`openTabBehind`, `tabs.ts:881`). `PlaneView.tsx` has 15 `openTab` and
  `openTabBehind` calls to review (for example `:4188`, `:5440`, `:5784`).
- **Upgrade (decision 8).** See section 7. This is the biggest surprise.
- **Sidebar (decision 5).** "This tab" is one scope row (`chatsScope.ts:118-125`) and a set
  (`tabSessions`, `chatsScope.ts:73`, `ChatsSection.tsx:374-394`). A pick stored as `tab` needs
  mapping to `workspace`.
- **Danger cue (decision 2).** `ends-a-chat` is in `LayoutPanes.tsx:923`, `StripMarks.tsx:114`,
  `paneFrame.css:396`, `cues.css:8-15`, and `pane.close` is titled "End this pane's chat"
  (`actions.ts:4687`).

## 7. Decision 8 can't be done as written from today's record

"Tabs with a split, a view or a pin stay" assumes the record knows about those tabs. It knows only
part of it:

- **Splits of plain chats:** already lost at every relaunch. They come back as separate tabs, so an
  upgrade finds them as plain one-chat tabs and closes them, unless it reads `Chat.beside`.
- **`beside` gives membership only.** It can group a tab's chats, but not the shape, direction or
  sizes, and the first launch can't restore more than a flat row.
- **A view beside a chat:** nothing recorded, so it can't stay.
- **Pins:** recorded on the chat or view, read through the first pane (`PlaneView.tsx:2086-2094`).

The fix is one new thing in the record, which a persistent layout needs anyway: **the tab, as a
tree**. That also serves the "reloaded window" case, since a reload reads the same record
(`lib.rs`, `tab_shows` comment). It's a change in the core (`reopen.rs`), with tests, and it needs an
owner in the spec.

Pane numbers would need a decision too. `named.panes` isn't recorded (`tabs.ts:347`), and a launch
renumbers them, so anything keyed on a pane number is launch-local today.

## 8. One session per tab, and everything keyed on a tab's session

**One session per tab isn't the code's rule.** Splits have put several sessions in a tab for a long
time, and a pane already shows a chat that isn't its own. The rules that exist are these three.

**A. A tab has a lead chat: its first pane.** `chatOf` (`tabs.ts:649`) and its neighbours. Under
"a chat isn't a tab", these need a decision each. Decision 9 makes most of them move off the tab:

- Tab name: `renameTab` (`tabs.ts:408`), `showInstead` (`:598`), `followRename` (`:823`).
- Pin: `isPinned` (`PlaneView.tsx:2086`), `pinTab` (`:5229`), `Chat.pinned`.
- Workspace strip: `workspaceOf` is the first pane's (`tabs.ts:964`), so a tab with chats from two
  workspaces is filed by its first. With free drag that becomes common, not rare.
- State mark and chips on the strip: `ChatStrip.tsx:62`, `:224`, `:322-323`, `:340`, `:397-398`,
  `:407`, `:447`, `:460`, `:512-513`.
- Tab's menu and the catalogue: 10 `chatOf` rows in `actions.ts:1999-2206`, tab close at
  `:3071-3106`, the task-tab test at `:3075`.
- `PlaneView.tsx` tab actions, including rename: `:5332`, `:5348`, `:6143`, `:6220`, `:6284`, `:6571`, `:6652-6817`, `:7545`, `:7593`,
  `:7702`, `:8742`, `:8868`, `:10130`.
- Task-or-session reading: `placedOf` (`tabs.ts:1487`), `shownName` (`PlaneView.tsx:11004`),
  `moveToOwnTab` (`tabs.ts:1609`).
- Who's "in front" for the core: `chatInFrontOf` (`tabs.ts:660`), told at `PlaneView.tsx:1496-1525`.

**B. A pane has an own chat that a task is shown in place of.** This is the deepest assumption.
Decision 1 lets a pane show any chat, which leaves `own` with no meaning, and most of #1486 and
#1489 depend on it:

- `Tab.shows` (`tabs.ts:327`), `showing` (`:1760`), `showOwn` (`:1371`), `showOwnIn` (`:1338`),
  `taskShownIn` (`:1396`), `layoutShown` (`:1406`), `shownLive` (`:1321`), `keepShown` (`:1436`),
  `restoreShown` (`:1457`), `leftBy` (`:1536`).
- `homeOf` and `ownedIn` (`tabs.ts:1279`, `:1299`), and everything through them: `switchTabTo`,
  `sessionOf`, `placeBeside`, `tasksPlacedBelow`, `panesMovedInto`.
- `tabChats.ts`: the tab's chats (`:100-143`), breadcrumbs (`:158`, `:198`), `hiddenNeeding` (`:248`).
- `LayoutPanes.tsx`: `others` (`:620`), so a pane's Notices include each hidden chat's, on that pane
  (`:339-372`). With no home, a chat in the sidebar has no pane to draw Notices on, so the Notices of
  chats off-screen need a new place. That's a cross-over with decision 7.
- The per-pane close logic: `PaneDoing` (`LayoutPanes.tsx:876-905`), `closePane`
  (`PlaneView.tsx:3642-3647`).
- The core: `Chat.shows` (`reopen.rs:188`), cleaned by `dispatched.rs:946-952` and
  `dispatchrestart.rs:270`.

**C. Code that reads "the tab holding a session" and takes the first match.** It's safe only because
a chat is on screen once, which decision 1 keeps as a rule. It wants a guard, not a change:

- `tabHolding` is written twice (`actions.ts:4769`, `PlaneView.tsx:10990`).
- `closeChat` (`tabs.ts:1172-1174`), `replaceSession` (`:1215`), `ownedIn` (`:1305`),
  `openViewBeside` (`:1705`).
- `paneDrawn` keeps one terminal per chat: a second pane on the same chat would replace the first's
  focus callback (`paneKeyboard.ts:20`, `:58`). It's keyed by chat, not by tab, so the rest of that
  file is fine. Note its header says a tab is switched to a chat, which is out of date after this.
- `SessionPane` gives the core one size per session (`SessionPane.tsx:356-357`), which is another
  reason for "never shown twice".

**Things that key on a tab or pane rather than a session, and are fine:** `paneBand` (by pane number,
`noticeSettle.ts:65`), the crumbs kept for a shown task (`tab:pane:session`, `PlaneView.tsx:10846`).

**Things that look at sets, and are fine:** `panesOf`-based ones such as `visibleSessions`
(`tabs.ts:1258`), wrapping marks (`ChatStrip.tsx:294-319`), `waitingOn` (`PlaneView.tsx:3000`),
`settings/signOutTerminals.ts:17`. They need only a check that a chat with no pane isn't dropped,
because they assume a running chat is in some tab. `PlaneView.tsx:1148` and `:1776` deserve a look:
the second feeds `chat_order`.

**In the core:**

- `Chats::looked_at` (`app/src-tauri/src/chats.rs:4707-4724`) decides what's on screen by
  `number == front` or `beside == Some(front)`, plus `shows`. It's what holds a reported task and
  withholds a notification (`:4703-4704`). So it needs to stay correct when panes change, and a tab
  with no lead chat or with a cleared first pane must still name its panes. It's the riskiest place
  to get wrong because a mistake is silent: a task ended under the person, or a notification lost.
- `Chat::has_tab` (`reopen.rs:280-287`), `tab_opened` (`:168`), `chats.rs:4768`, `:4799`,
  `open_chat_tab` and `close_chat_tab` (`lib.rs:2629`, `:2642`).

## 9. What I couldn't determine

- **What a pane draws after its program exits.** The chat and its pane stay (section 5), but I
  didn't trace what the person sees on that terminal.
- **Whether sizes carry between tabs** (section 2). It's inferred from React's rules and the missing
  `key`. Needs a two-tab run.
- **What a separator does at 0%**, and how a terminal behaves there. The library's floor and
  `fit.fit()` at very small sizes weren't exercised.
- **The library's `Group` behaviour on a changed child.** I read its types, not its code, so what
  `defaultSize` does on a mounted panel is taken from the comment at `RegionFrame.tsx:396-397`.
- **A drag across a terminal.** Whether xterm and the Tauri webview let a pointer drag cross a pane
  without help wasn't tested. `tauri.conf` has no `dragDropEnabled` setting I could find in
  `app/src-tauri`, and reference drags do work, so I assume the HTML route is clear.
- **The full `openTab` list.** I reviewed the `PlaneView.tsx` calls, not every `tabs.ts` consumer
  outside it. The site lists in section 8 come from a grep for the helpers named there; a chat lookup
  written by hand wouldn't show. In the same way, the helper grep was run over `app/src` and the
  `chats.rs` reads are the ones I followed, not an exhaustive list.
- **Tests that pin the old model.** `tabs.test.ts` and `pane-crumbs.e2e.ts` weren't surveyed. The
  rewrite will move a lot of them.
- **Other windows.** #4575 lists split windows as unspecified; I didn't check them beyond the note
  in `homeOf` that a chat may be "in another window" (`tabs.ts:1276-1277`).
