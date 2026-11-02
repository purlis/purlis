# The command table after #4585 (PROTOTYPE, throwaway)

Map #4575 "Chats, tabs and panes: a chat isn't a tab", ticket #4585. This is the table #4577 counted
(`research/tab-commands.md` on branch `research-tab-commands`), redrawn to the rule in #4576 D9. Row
numbers are #4577's, and new rows are marked **new**. This note is the spec's input. It is not code.

## The rules the table follows

1. **One list per chat.** The pane head's ⋯, the sidebar row's ⋯ and right-click, and the palette all
   read one ordered list of a chat's rows: one catalogue function, one `menuOn` entry (`on: "chat"`,
   by session and not by tab). The palette's word is the menu's word with the chat's name put in.
   A test lists every chat row and fails if a row is missing from any of the three.
2. **Kind hides, state disables.** A row that can't mean anything for this kind of chat is left
   out: a shell has no Ask, a task has no End. A row that applies but can't run now is shown
   greyed with its reason, for example Start fresh when the instructions haven't changed.
3. **The list's order:** rows for the chat's state now (Answer, Cancel smart close, Dismiss,
   Resume), then the chat (Rename, Use skill, Ask, Skills, Harness, Link), then open (Open in new
   tab, Open beside, Brief, Go to, Activity, Network), then run (Restart, Start fresh, Move to,
   Stop ▸), then Branch, then the end (End…, or a task's Stop and get its report and Close now).
4. **The pane head**, left to right: the icon and name, a state mark, "harness · workspace", a gap,
   the chat's part (variant A: at most one state button, then ⋯), then the bell slot (#4584), then
   Split right, Split down and Close pane. A narrow pane drops parts in a fixed order: "harness ·
   workspace" first (under 520px), the state mark's word (470), the state button's word (420), the
   mark (380), then Split down (300). The bell, ⋯, Split right and Close never go.
5. **Right-click on a pane head** shows the chat's list, a line, then the pane's rows. **Right-click
   on a tab** shows only tab rows. No chat row is on a tab, so a split tab can't act on the wrong
   chat any more (#4577 found it acted on the first pane's chat).
6. **A tab and a pane never end a chat.** Close tab and Close pane only clear. End… is the chat's own
   row, and it asks first (EndingChat).

## Chats (the one list)

| # | Row (menu word) | Palette word | Pane head (A) | Shell | Ended chat | Change from today |
|---|---|---|---|---|---|---|
| 30 | Answer its question | `Answer X's question` | **direct** as `Answer` while a task asks | — | — | moves off the tab menu; now a pane button |
| 16 | Cancel smart close | `Cancel smart close of X` | **direct** while it wraps up | — (no Smart close) | — | off the tab and its chip, onto the chat's surfaces |
| 22 | Dismiss: smart close stopped | `Dismiss X: smart close stopped without its record` | ⋯; also the band notice | — | — | was palette only |
| new | Resume | `Resume chat X` | the ended pane's bar (#4587) | yes | **only here** | new |
| 1 | Rename… | `Rename chat X…` | ⋯ | yes | yes | off the tab; adds pane and row |
| 6 | Use skill… | `Use skill in X…` | ⋯ (C: icon) | **no** | no | off the tab; every chat, not only the tab in front |
| 8 | Ask ▸ persona | `Ask a persona from X…` | ⋯ | **no** | no | off the tab |
| 7 | Skills (N) | `Show X's skills` | ⋯ (the head's chip goes) | **no** | yes | was pane only |
| 9 | Harness · model | `<Harness> card of X` | ⋯ (the head says "harness · workspace") | **no** | yes | adds the sidebar row |
| 2, 3 | Link to work item… / Unlink #N | `Link X to a work item…` / `Unlink X from #N` | ⋯ | yes | yes | off the tab |
| new | Open in new tab | `Open X in a new tab` | ⋯; the row's hover icon (#4582) | yes | yes | replaces 31 Move to its own tab |
| 32 | Open beside Y (a task) | `Open X beside Y` | ⋯ | — | yes | off the task's tab |
| 29 | Brief (a task) | `Brief of X` | **direct** on a quiet task | — | yes | unchanged |
| 18 | Go to Y (a handoff) | `Go to Y (handed off to by X)` | ⋯ | — | yes | adds the pane |
| 4, 5 | Activity, Network | `Activity of X`, `Network of X` | ⋯ (C: Activity icon) | yes | yes | off the tab's Open ▸ |
| 10 | Restart (`Restart when this turn ends` mid-turn) | `Restart chat X` | ⋯ (C: icon) | **no** | greyed: Resume picks it up | adds the pane and every row, not only tabless ones |
| 11 | Start fresh | `Start chat X fresh` | ⋯, greyed unless instructions changed | **no** | no | off the tab and its fresh mark |
| 17 | Move to ▸ pool | `Move chat X to…` | ⋯ | **no** | no | was palette only |
| 13–15 | Stop ▸ Stop · Stop and everything below it · Stop all tasks | `Stop chat X`, … | **direct** `Stop` while it works; ⋯ | Stop only | no | adds the pane |
| 24, 25 | Merge its branch · Remove its branch folder | `Merge X's branch into its clone`, … | ⋯ Branch | yes, in a branch folder | yes | were palette only |
| 26 | Discard and remove anyway | (in the refusal's own dialog) | — | | | stays where the refusal is shown |
| 27, 28 | Stop and get its report · Close now (a task) | `Stop and get its report: X`, `Close now: X` | ⋯ | — | — | the head's `Stop` and `Close now` buttons become ⋯ rows (A) |
| 12 | End… (Cancel · End · Smart close) | `End chat X…` | ⋯ (C: icon) | yes; asks only Cancel or End | — | off the tab's `×`, ⌫ and menu; adds the sidebar row |
| 21 | Ignore until it asks again | `Ignore X until it asks again` | ⋯ while it needs you | yes | — | was palette, title bar and Inbox only |

**Not on a shell** (#4588): Use skill, Ask, Skills, Harness, Restart, Start fresh, Move to, Stop and
everything below it, Stop all tasks, Smart close, and the task rows. A shell keeps Rename, Link, Open
in new tab, Activity, Network, Stop, Branch rows and End.

**Gone:** 31 Move to its own tab (now Open in new tab), 33 Send back (Close pane does it; the task
keeps running under its chat in the sidebar), 19 and 23 Show (the row's press, and the palette's
`Show X`, which stays as the row press's keyboard way).

## Smart close (ADR 0064) after this

- **Where it lives:** still the third answer of End…'s dialog, so it's reached from every surface
  End… is on: pane ⋯, sidebar row, palette. It's never on a tab or a pane's ×.
- **The chip goes.** Today a wrapping-up chat's tab turns into a chip at the strip's left edge
  (`ChatStrip.tsx` `isBackground`, `StripMarks`). A chat has no tab now, so the chip has nothing to
  be. The wrapping-up look goes where the chat is: a breathing "Wrapping up" mark on its pane head
  (when on screen) and on its sidebar row, and Cancel smart close first in its list and as the
  head's state button. Clearing the pane while it wraps up is fine: it keeps going in the sidebar.
- **When the record is written** the chat ends like any End: if on screen, its pane keeps the last
  screen with Resume and Clear pane (#4587) and links the record. Nothing opens if it wasn't on screen.
- **When it stops without a record** (SI-8f): the pane's band gets the notice with End… and Dismiss,
  the bell counts it, the row goes needs-you, and Dismiss is on the list.

## Panes

| # | Row | Where | Change |
|---|---|---|---|
| 44, 45 | Split right, Split down | head buttons, head right-click, palette | unchanged |
| 46 | Close pane (the chat keeps running) | head `×`, head right-click, palette `Close this pane`, the ended bar's `Clear pane` | no longer ends a chat; no danger cue (D2) |
| new | Tile all | head right-click, tab right-click, palette | new (D6) |
| 47 | Send F2 | head and terminal right-click, palette | unchanged |
| 48–50 | Copy, Paste, Select all | terminal right-click, **palette** | adds the palette, so none is pane only |
| 42, 43 | New tab, New shell | strip `+` and the Chats heading, palette | off the pane's right-click: they're the window's |

## Tabs (what's left on a tab's right-click)

| # | Row | Where | Change |
|---|---|---|---|
| 35 | Pin tab / Unpin tab | tab menu, palette | the words say tab, not chat |
| new | Tile all | tab menu, head right-click, palette | new |
| 36 | Keep open (a preview tab) | tab menu, double-click, palette | adds the tab menu (was palette only) |
| 37 | Close other tabs | tab menu, palette | new |
| 37 | Close tab (its chats keep running) | tab menu, `×`, ⌫, palette | never ends a chat; `Close and end the chat beside it` goes |
| — | Move to new window | — | the map's "Several windows" fog |
| 34 | Switch to | the tab's press, palette | off the menu: the press does it |
| 38–41 | This tab's tasks, Next/Previous chat in this tab, Back to this tab's own chat | — | their meaning goes with tasks-in-a-tab; to the map's "Keyboard" fog |

## Other sidebar rows

51, 52 (a finished task's Merge… and Discard branch…) lose `menuOnly` so the palette has them too.
53–56 (browser rows) stay as they are.

## One surface only, after

None among chat, pane and tab rows. Of #4577's 19: 13 palette-only rows are on the list or moved to
the row press or to the Keyboard fog (38–40); 7, 48–50 gain the palette or the row; 51–52 gain the
palette. The test of D9 holds the chat list to three surfaces and pane and tab rows to two.
