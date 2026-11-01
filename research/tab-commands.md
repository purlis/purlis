# Tab, pane, sidebar-row and palette commands today

Question (#4577): what is every command on a tab's right-click menu, the pane header, the sidebar row's menu and the command palette, which of them acts on a chat, a tab or a pane, and which exist on only one surface? Read at commit `7e723d5d4`. This is a throwaway research note for #4577 under map #4575, the input to the rule in #4576. It reports today's state and designs nothing. The app was not run: everything here is from reading source.

## How the surfaces are built

Three of the four surfaces read one list. `app/src/actions.ts` `catalogue()` (A:1740) builds every row; the palette lists it, and a context menu is `menuOn()` (A:4987), a list of row ids per kind of thing, looked up in that catalogue by `menuRows()` (A:5305) and given short words by `menuModel.ts`.

| Surface | Definition | Static? |
|---|---|---|
| Tab right-click | `Menued on={{ on: "chat", tab, session: chatOf(tabs, id) }}` CST:223-227; rows A:4993-5027; the Ask submenu MM:545-553. The same `on: "chat"` for every kind of tab, so a tab's kind shows only in which ids the catalogue has. | Ids per tab |
| Pane header | Buttons: `PaneDoing` LP:876-934, `TaskEnds` TE:322, `BriefButton` BR:83, `SkillsChip` US:447, `HarnessChip` HC:51, breadcrumb `PaneCrumbs` PC:42. There is no kebab or overflow menu. Right-click: the panes' menu `Menued on={{ on: "pane" }}` PV:9859, rows A:5171-5175; on a task's breadcrumb, the Chats row's menu LP:221-227, 731-739; on a terminal, `TerminalMenu` TM:81-84 (own rows, then the panes' rows). | Conditional per pane |
| Sidebar row | Three kinds of row in the Chats view, each with its own menu: chat row `on: "listed"` CS:1548, rows A:5028-5047; finished task `on: "finished"` FT:427, rows A:5165-5167; browser `on: "browser"` CB:234-243, rows A:5117-5125. Right-click only (or Shift+F10, registry `row.menu` keys/registry.ts:711): rows have no ⋯ button. | Ids per row |
| Palette | `Palette.tsx`, listing `inPalette(offers)` PAL:388, A:4775. Offers are the project in front's catalogue, with the needs-you queue (`withQueue` PV:8659) and session rows (`withSessionRows` App.tsx:1346) put in; with no project, the window's opener rows App.tsx:2782. | See below |

**Palette assembly.** Rows are a flat list, filtered and ranked by typing (`narrow`, A:4867). There are no per-chat search results with actions under them. Three kinds of row exist: static (`Split right`), one row per named thing (`Rename chat X…`, one per tab, whichever tab is in front), and rows contextual to what is in front (`pane.*`, `tasks.*`, `worktree.merge`, `worktree.remove`). Rows marked `menuOnly` are left out (A:4775): Ask and Use skill for tabs not in front (A:2074, 2103, 2111), and a finished task's Merge and Discard (A:2471, 2480). A second scope lists only projects and files (PAL:147, 388); it holds no chat, tab or pane rows.

**Citation key.** Paths are under `app/src/`. A `actions.ts`; MM `menuModel.ts`; MW `menuWords.ts`; LP `LayoutPanes.tsx`; PC `PaneCrumbs.tsx`; CS `ChatsSection.tsx`; CST `ChatStrip.tsx`; PV `PlaneView.tsx`; TM `TerminalMenu.tsx`; TR `terminalRows.ts`; TE `TaskEnd.tsx`; BR `Brief.tsx`; US `UseSkill.tsx`; HC `HarnessCard.tsx`; FT `FinishedTasks.tsx`; CB `ChatBrowsersShown.tsx`; TC `TabChip.tsx`; SM `StripMarks.tsx`; PAL `Palette.tsx`.

**Labels.** There is no i18n: words are literals. A tab or row label computed by `unnamed()` (MM:141-155) takes the name out of the catalogue title; those were worked out by hand from that function. `X` is the chat's or tab's name, `Y` another chat's. The palette's label is the catalogue title.

**Pane header cell tags.** [btn] a button on the header line; [ctx] the panes' right-click menu; [crumb] right-click on a task's breadcrumb; [term] right-click on a terminal. [ctx] rows also appear under [term] (LP:99-107). A row with [crumb] is only there while the pane's path ends in a task (LP:732). **★** marks a row on exactly one of the four surfaces.

## The table

### Chats

| # | Command | Acts on | Tab menu | Pane header | Sidebar row | Palette | One surface only? |
|---|---|---|---|---|---|---|---|
| 1 | Rename | chat (held as the tab's name) | `Rename…` A:4998, MM:193, MW:19. Tab whose first pane is a chat A:2020 | — | — | `Rename chat X…` A:2022 | no |
| 2 | Link to work item | chat | `Link to work item…` A:5000, MM:195, MW:23. Chat filed in a workspace A:2207, PV:7796 | — | — | `Link to work item…` A:2210 | no |
| 3 | Unlink work item | chat | `Unlink work item` A:5001, MM:196, MW:24. Chat with a link A:2214 | — | — | `Unlink work item` A:2216 | no |
| 4 | Activity | chat | Open ▸ `Activity` A:5008, MM:204, MW:28, 78 | — | — | `Activity` A:2034 | no |
| 5 | Network | chat | Open ▸ `Network` A:5009, MM:205, MW:29, 78 | — | — | `Network` A:2050 | no |
| 6 | Use skill | chat | `Use skill…` A:5010, MM:207. Skill studio on, not a shell A:2065, 2069 | — | — | `Use skill…` A:2072. Tab in front only A:2074 | no |
| 7 | Show this chat's skills | chat | — | [btn] `N skills`, named `N skills. Show this chat's skills.` US:464, 473, 481; LP:238. Studio on, not on a task's path, pane wider than 40rem | — | — | ★ pane |
| 8 | Ask a persona | chat | Ask ▸ `<persona>` MM:545-553, A:5249-5255, MW:79. Runnable rows only; not a shell | — | — | `Ask <persona>…` A:2102, 2108, tab in front only A:2111; `Ask a persona…` (cannot run) where policy locks all A:2088 | no |
| 9 | Harness card | chat's harness | — | [btn] the card's label, HC:60-66, LP:239. Chat with a harness glance | — | The card's label A:1914-1919 | no |
| 10 | Restart | chat | `Restart`, mid-turn `Restart when this turn ends` A:5013, MM:208, MW:30-31. Not a shell A:2163, PV:6187 | [crumb] `Restart` A:5041, MM:209 | `Restart` A:5041, MM:209. **Only a chat with no tab** A:4182 | `Restart chat X` A:2166 (tab), A:4184 (no tab); mid-turn `Restart chat X when this turn ends` A:2168, 4186 | no |
| 11 | Start fresh | chat | `Start fresh` A:5014, MM:215, MW:32. Only where project instructions changed A:2125. Tab's fresh mark runs the same row CST:455-462 | — | — | `Start chat X fresh` A:2138, 2149 | no |
| 12 | End chat (`tab.close` on a chat's tab) | chat and tab: closes the tab and ends all its non-task chats PV:3664-3679 | `End`, from `End chat X` A:5015, MM:216-224. Cannot run while restoring A:3097. Also the tab's `×` CST:463 and ⌫ CST:359 | — (see row 46) | — (see row 13) | `End chat X` A:3100-3105 | no |
| 13 | Stop chat | chat | Stop ▸ `Stop`, or `End now` while stopping, A:5021, MM:175-182, 210, MW:83. Not a task A:3944 | — | Stop ▸ `Stop` / `End now` A:5042, MM:210 | `Stop chat X` A:3989; `End chat X now` A:3954 | no |
| 14 | Stop chat and everything below | chat tree | Stop ▸ `Stop and everything below it` A:5022, MM:211. Cannot run with nothing below, A:4000 | — | Same A:5043, MM:211 | `Stop chat X and everything below it` A:3947 | no |
| 15 | Stop all tasks | chat's tasks | Stop ▸ `Stop all tasks` A:5023, MM:212, MW:33. Cannot run with none open A:4012 | — | Same A:5044 | `Stop all tasks of X` A:3948 | no |
| 16 | Cancel smart close | chat | `Cancel smart close` A:4996, MM:191, MW:25. Tab wrapping up A:2237-2241, including the background chip CST:229-233 | — | — | `Cancel smart close of X` A:2246 | no |
| 17 | Move chat to a pool, or here | chat | — | — | — | `Move chat X to pool P`, `Move chat X here`, with `with …` naming chats that go too, A:2189-2192. In no `menuOn` list; no other caller builds the row (grep of `tab.move:`) | ★ palette |
| 18 | Go to the chat it handed off to | other chat | — | — (the breadcrumb's menu passes no `handed`, LP:222, A:5034) | `Go to Y` A:5034, MM:198. Chat that handed off | `Go to Y (handed off to by X)` A:4147 | no |
| 19 | Show a queued chat | chat | — | — | Listed as `Show` A:5031, MM:197, MW:36, **but not drawn**: see "What I couldn't determine" | `Show X, which needs you`, or `Show X: <why>` A:3800-3807, 3845 | ★ palette |
| 20 | Show the next chat that needs you | chat | — | — | — | `Show the chat that needs you` A:1961-1962 | ★ palette |
| 21 | Ignore until it asks again | chat's request | — | — | — | `Ignore X until it asks again` A:3817. Also the title bar's queue and the Inbox | ★ palette |
| 22 | Dismiss a stopped smart close | chat's notice | — | — | — | `Dismiss X: <why>` A:3851 | ★ palette |
| 23 | Show a task | chat (task) | — | — | — (pressing the row does this, CS:1571) | `Show task X in W, asked by Y` A:4124 | ★ palette |
| 24 | Merge this chat's branch | chat's branch | — | — | — | `Merge this chat's branch into its clone` A:2983-2987 | ★ palette |
| 25 | Remove this chat's branch folder | chat's branch | — | — | — | `Remove the folder of this chat's branch` A:3120-3131 | ★ palette |
| 26 | Discard and remove anyway | chat's branch | — | — | — | `Discard that work and remove the folder anyway` A:3164. Only after a refused removal A:3162 | ★ palette |

### Tasks (a chat another chat asked for)

| # | Command | Acts on | Tab menu | Pane header | Sidebar row | Palette | One surface only? |
|---|---|---|---|---|---|---|---|
| 27 | Stop and get its report | chat (task) | `Stop and get its report` A:5024, MM:213, MW:34. Task's own tab only | [btn] `Stop`, named `Stop and get its report: task X`, TE:309, 380, LP:718-729; [crumb] `Stop and get its report` A:5045 | `Stop and get its report` A:5045; Delete on the row CS:1458 | `Stop and get its report: task X` A:4073 | no |
| 28 | Close now | chat (task) | `Close now` A:5024, MM:214, MW:35. Task's own tab only | [btn] `Close now`, named `Close now: task X`, TE:309, 380; [crumb] `Close now` | `Close now` A:5045 | `Close now: task X` A:4074 | no |
| 29 | Brief | chat (task) | `Brief` A:5005, MM:202, MW:26. Task's own tab only | [btn] icon named `Brief of X` BR:93, LP:231; [crumb] `Brief` | `Brief` A:5032, MM:202 | `Brief of X` A:4204 | no |
| 30 | Answer its question | chat (task) | `Answer its question` A:5007, MM:203, MW:27. Task asking its asking chat A:4222 | [crumb] `Answer its question` A:5033 | `Answer its question` A:5033 | `Answer X's question` A:4226 | no |
| 31 | Move to its own tab | chat (task) | — | [btn] icon named `Move X to its own tab` LP:902, 920, SM:424, while a pane shows it inside its session's tab LP:641; [crumb] `Move to its own tab` | `Move to its own tab` A:5036, MM:199, MW:37 | `Move X to its own tab` A:4342 | no |
| 32 | Open beside its session | chat (task), splits the session's tab | `Open beside Y` A:5003, MM:200. Task's own tab only | [btn] icon named `Open X beside Y` LP:902, 920, while shown in its session's tab; [crumb] `Open beside Y` | `Open beside Y` A:5037; Space on the row CS:1457 | `Open X beside Y` A:4343-4346; `Open the task in front beside its session` A:4392 | no |
| 33 | Send back | chat (task) | `Send back into Y's tab`: `tab.close` on a task's own tab, drawn above the line A:3078-3085, 5317, MM:220 | [btn] the pane's `−` named `Send X back …` LP:900, A:4666-4675, SM:422; [crumb] `Send back …` A:5038 | `Send back into Y's tab`, `Send back among Y's tasks`, or `Send back to the Chats list` A:5038, MM:201, A:4315-4318. While it has a tab or pane of its own A:4368 | `Send X back …` A:4372, and two more rows of the same words, `tab.close` A:3080 and `pane.close` A:4670 | no |

### Tabs

| # | Command | Acts on | Tab menu | Pane header | Sidebar row | Palette | One surface only? |
|---|---|---|---|---|---|---|---|
| 34 | Switch to tab | tab | `Switch to` A:4997, MM:192, MW:17. Left out on the tab in front | — | — | `Switch to tab X` A:1982-1986; cannot run on the tab in front | no |
| 35 | Pin | tab (stored with its chat or view, A:1996-1998) | `Pinned`, a toggle with a tick, A:4999, MM:174, MW:20 | — | — | `Pin chat X`, `Unpin chat X`, `Pin tab X` on a view's tab A:2008 | no |
| 36 | Keep preview tab open | tab | — (in no `menuOn` list). Double-click on the tab CST:388 | — | — | `Keep tab X open` A:2230. Preview tab only A:2227 | ★ palette |
| 37 | Close a tab with no chat of its own | tab | `Close`, from `Close X` A:3090; or `Close and end the chat beside it` A:3102, MM:216-224 | — (see row 46) | — | `Close X` A:3090, `Close X and end the chat beside it` A:3102 | no |
| 38 | Show this tab's tasks | tab | — | — | — | `Show this tab's tasks` A:4711. Key: registry `tasks.menu` | ★ palette |
| 39 | Next chat in this tab | tab | — | — | — | `Next chat in this tab` A:4712. Key: `tasks.next` | ★ palette |
| 40 | Previous chat in this tab | tab | — | — | — | `Previous chat in this tab` A:4713. Key: `tasks.previous` | ★ palette |
| 41 | Back to this tab's own chat | tab | — | [btn] the first name on the breadcrumb PC:77-85 | — | `Back to this tab's own chat` A:4714. Key: `tasks.own` | no |
| 42 | New tab | window | — (strip `+` CST:485) | [ctx] `New tab` A:5173, MM:330 | — (Chats heading `New chat` CS:945, not a row's menu) | `New tab` A:1753 | no |
| 43 | New shell | window | — | [ctx] `New shell` A:5173, MM:233, MW:38 | — (Chats heading More ▸ `New shell` CS:1251) | `New shell` A:1759 | no |

### Panes

| # | Command | Acts on | Tab menu | Pane header | Sidebar row | Palette | One surface only? |
|---|---|---|---|---|---|---|---|
| 44 | Split right | pane | — | [btn] icon named `Split right` LP:903, 920; [ctx] `Split right` A:5173, MM:331 | — | `Split right` A:1925, 1928-1931 | no |
| 45 | Split down | pane | — | [btn] icon named `Split down` LP:904, 920; [ctx] `Split down` A:5173, MM:332 | — | `Split down` A:1926, 1928-1931 | no |
| 46 | Close the pane | pane, and the chat in it | — | [btn] icon named `End this pane's chat`, or `Close this view` on a view's pane, LP:905, 910, A:4656, 4687. No button on a pane showing a task of its tab's session LP:638, 905. [ctx] same words A:5174, MM:334-339, for the focused pane | — | `End this pane's chat` A:4687, `Close this view` A:4656; the focused pane's. Cannot run on a pane showing a task A:4678-4684 | no |
| 47 | Send F2 | pane's chat or driven page | — | [ctx] `Send F2 to the chat in front` A:5173, MM:333. On a driven browser, `Send F2 to the page you are driving` | — | Same A:1937-1939 | no |
| 48 | Copy | terminal | — | [term] `Copy` TR:84-85 | — | — | ★ pane |
| 49 | Paste | terminal | — | [term] `Paste` TR:91-92 | — | — | ★ pane |
| 50 | Select all | terminal | — | [term] `Select all` TR:98-99 | — | — | ★ pane |

### Other kinds of sidebar row

| # | Command | Acts on | Tab menu | Pane header | Sidebar row | Palette | One surface only? |
|---|---|---|---|---|---|---|---|
| 51 | Merge a finished task's branch | other (finished task) | — | — | `Merge…` A:5167, MM:316, A:2469 | — (`menuOnly` A:2471) | ★ sidebar |
| 52 | Discard a finished task's branch | other (finished task) | — | — | `Discard branch…` A:5167, MM:317, A:2474 | — (`menuOnly` A:2480) | ★ sidebar |
| 53 | Browser timeline | other (browser, of a chat) | — | — | `Browser timeline of X` A:5122, 2758. Drawn by title. Chat still open CB:240-242 | `Browser timeline of X` A:2758 | no |
| 54 | Pin browser | other (browser) | — | — | `Pin browser B of X` / `Unpin …` A:5120, 2771. Drawn by title | Same A:2771 | no |
| 55 | Hand browser to a chat | other (browser) | — | — | `Hand browser B of X to Y` A:5121, 2786-2788. A pinned browser whose chat closed | Same | no |
| 56 | Stop browser | other (browser) | — | — | `Stop browser B of X` A:5124, 2798. Same condition A:2767-2779 | Same A:2798 | no |

Totals: 56 rows. 19 on exactly one surface. 7 on all four (rows 10, 27-30, 32, 33), several of those only by a right-click on a breadcrumb.

## Which rows a tab or a row actually has

Every tab gets the same menu definition. What differs is which ids the catalogue holds:

| Kind of tab | Rows |
|---|---|
| Chat's tab | Rename, Pinned, Link, Open ▸, Use skill, Ask ▸, Restart, End, Stop ▸, plus Switch to when not in front; Start fresh, Unlink, as conditions in the table |
| Shell's tab | The same less Use skill, Ask and Restart (A:2069, 2092, PV:6187) |
| Task's own tab | Adds Open beside, Brief, Answer, Stop and get its report, Close now; no Stop ▸ (A:3944); `tab.close` becomes Send back and is above the line |
| View's or file's tab | Switch to, Pinned (`Pin tab X`), Close. `chatOf` is undefined, so no chat row and `session` is undefined (A:649-652) |
| Tab with a view first and a chat beside | As a view's tab, with `Close and end the chat beside it`. No chat row reaches the chat beside it |
| Split chat tab | Every chat row is about the **first** pane's chat (`chatOf`, tabs.ts:649), not the focused one. End ends every non-task chat in it |
| Tab showing a task inside it | The tab's menu is its session's, not the task's on screen (CST:224). Only the pane offers the task's rows |
| Wrapping-up tab, drawn as a chip | The same menu, with Cancel smart close first (CST:233) |
| Restoring tab | End cannot run, says why (A:3096-3097) |

Sidebar rows in the Chats view:

| Kind of row | Menu |
|---|---|
| Top-level chat or handoff with a tab | Stop ▸ and `Go to Y` for each handoff. That is the whole menu: no Rename, no Restart, no End |
| Shell | Same (`restartRows` excludes shells A:4182) |
| Task | Brief, Answer, Move to its own tab, Open beside, Send back, Restart if it has no tab, Stop and get its report, Close now |
| Finished task | Merge…, Discard branch…, only with a branch of its own and not waiting to start (A:2467). Everything else is an inline button: Brief, Reopen, Review changes or Changes, Clear; or for one that did not start, Try to start again, Review and approve…, End task, End it, Keep (FT:257-371). None of these buttons is a catalogue row |
| Browser | Rows 53-56 |

Explorer, Sessions, Personas and the other sidebar lists have their own menus (Explorer.tsx:1517, 1676; Panels.tsx:548-571) about clones, branches, files, records and personas, not about a chat. The workspace strip (App.tsx:2373-2380) and project rail (ProjectRail.tsx:306) are tabs of another axis, read here as out of scope.

## Palette rows that aren't about a chat, tab or pane

88 id families by hand count, all from `catalogue()` (A:1740-3284) unless noted. The count depends on where a family is drawn, so read it as approximate. Most expand per item at runtime.

- Window, 13: `browser.new`, `extensions.show`, `search.files`, `view.show:*`, three `view.toggle:*`, `view.move:*`, two `view.region:*`, `focus.toggle`, `shortcuts.show`, `terminals:screen-reader` (A:1766-1908).
- Settings and machine, 7: `settings.show`, `settings.you`, `pool.add`, `devices.show`, `settings.group:*` (A:3698-3770), `charter.installCli`, `charter.quit`.
- Project, 10: `projectRows` (A:1221), spliced at A:2409-2417, 3270.
- Workspace, 11 in the palette; persona, 6; memory, 7; dispatches, browsers and skill studio, 3; vault, 4; todo, 5; clone, 4; not-cloned repo, 3; extension views and commands, 2; curation, 1; branch folder, 9 (`worktree.*:<piece>`); read again, 1 (A:2257-3050, 3134-3260).
- Session records, 2 (`sessionOffers` A:1674, put in at App.tsx:1346). `session.resume` is the closest thing to reopening a closed chat: the Sessions panel's menu and the palette have it, and no chat surface does.

## One surface only

- **Palette (13):** 17 Move chat, 19 Show a queued chat, 20 Show the next chat that needs you, 21 Ignore, 22 Dismiss, 23 Show a task, 24 Merge this chat's branch, 25 Remove its folder, 26 Discard, 36 Keep tab open, 38 Show this tab's tasks, 39 Next chat, 40 Previous chat.
- **Pane (4):** 7 Show this chat's skills, 48 Copy, 49 Paste, 50 Select all.
- **Sidebar (2):** 51 Merge…, 52 Discard branch…
- **Tab menu (0):** every row of a tab's menu is also in the palette, though Ask and Use skill only for the tab in front.

Several of the 13 have a way in outside these surfaces, by key, title bar, row press or tab chip. Those are noted in the table and do not change the count.

## Wording differences

The palette's words carry the name because a row stands alone there; the menus drop it by design (MW:7-11). Differences beyond dropping the name are in bold.

| Row | Tab menu | Pane header | Sidebar row | Palette |
|---|---|---|---|---|
| 1 Rename | `Rename…` | — | — | `Rename chat X…` |
| 8 Ask | Ask ▸ `<persona>` | — | — | `Ask <persona>…` |
| 10 Restart | `Restart` | `Restart` | `Restart` | `Restart chat X` |
| 11 Start fresh | `Start fresh` | — | — | **`Start chat X fresh`** |
| 12, 13 End and Stop | **`End`** and **`Stop`** | **`End this pane's chat`** | **`Stop`**, no End | `End chat X`, `Stop chat X` |
| 15 Stop all tasks | `Stop all tasks` | — | same | `Stop all tasks of X` |
| 16 Cancel smart close | `Cancel smart close` | — | — | `Cancel smart close of X` |
| 18 Go to | — | — | `Go to Y` | **`Go to Y (handed off to by X)`** |
| 27 Stop task | `Stop and get its report` | **`Stop`** (named in full) | `Stop and get its report` | `Stop and get its report: task X` |
| 28 Close now | `Close now` | `Close now` | `Close now` | `Close now: task X` |
| 29 Brief | `Brief` | `Brief of X` (icon) | `Brief` | `Brief of X` |
| 30 Answer | `Answer its question` | same | same | `Answer X's question` |
| 31 Own tab | — | `Move X to its own tab` | `Move to its own tab` | `Move X to its own tab` |
| 32 Beside | `Open beside Y` | `Open X beside Y` | `Open beside Y` | `Open X beside Y`, **`Open the task in front beside its session`** |
| 33 Send back | `Send back into Y's tab` | `Send X back into Y's tab` | same as tab | same as pane |
| 35 Pin | **`Pinned`** (tick) | — | — | `Pin chat X`, `Unpin chat X`, `Pin tab X` |
| 37, 46 Close | `Close` | **`Close this view`** | — | `Close X`, `Close this view` |
| 42 New tab | — | `New tab` | **`New chat`** on the heading | `New tab` |

Also: `Stop chat X` and `End chat X` are different verbs that read alike. Stop asks, then the core stops the chat and a dispatched chat gets a short turn to write what it did (A:4421, PV:2230-2258). End closes the tab (PV:3664-3679) after the dialog in `EndingChat.tsx`. Their menu warnings are `Ends the chat.` (MW:97) and `Ends the program it runs. There is no undo.` (MW:95). `Split right` and `Split down` are the only rows with one wording everywhere. Activity, Network and the work-link rows match across surfaces only because the palette's title carries no name, so with many tabs it lists several identical `Activity` rows told apart by their note (A:2039).

## Misplaced by the #4576 rule

**On a tab's right-click, and not a tab thing** (18 rows): 1 Rename, 2 Link, 3 Unlink, 4 Activity, 5 Network, 6 Use skill, 8 Ask, 10 Restart, 11 Start fresh, 13 Stop, 14 Stop and below, 15 Stop all tasks, 16 Cancel smart close, 27 Stop and get its report, 28 Close now, 29 Brief, 30 Answer, 32 Open beside. Row 12 End and row 33 Send back are the unclear cases: both are the tab's close, doing a chat's ending or a task's move. Left on it as tab things: 34 Switch to, 35 Pinned, 37 Close.

Row 1 is a judgement call: the palette calls it `Rename chat`, and a view's tab has none, so I classed it as a chat's.

**Chat actions missing from one of the three chat surfaces** (pane header, sidebar row, palette):

| Row | Missing from |
|---|---|
| 1 Rename, 2 Link, 3 Unlink, 4 Activity, 5 Network, 6 Use skill, 8 Ask, 11 Start fresh, 16 Cancel smart close | Pane header and sidebar row |
| 10 Restart | Pane header, except a task's breadcrumb; sidebar row for any chat with a tab |
| 12 End chat | Sidebar row. The pane has its own close (row 46) |
| 13, 14, 15 Stop rows | Pane header |
| 17 Move chat | Pane header and sidebar row, and the tab menu |
| 18 Go to | Pane header |
| 19-23 Show, Ignore, Dismiss | Pane header and sidebar row |
| 24-26 Branch rows | Pane header and sidebar row |
| 30 Answer | Has no pane button; only a breadcrumb's right-click |
| 7, 9 Skills chip, harness card | 7 is pane only; 9 is missing from the sidebar row |

The task rows 27-33 are the nearest to the rule today: they are one catalogue row on every surface and the breadcrumb reuses the sidebar's menu by id (LP:80-91).

**On the pane and not split, stacked or close:** the [btn] rows Brief, Stop, Close now, Move to its own tab, Open beside, the chips; the [ctx] rows New tab, New shell, Send F2; and rows 48-50. The pane's close is also not a pane-only act: it ends a chat.

## Adjacent surfaces, not counted

- **Tab chip menu** (`TabTasks` TC:263). Beside a tab with tasks, a fifth place with its own hand-written rows: `End a task` ▸ the catalogue's titles, `Stop all tasks` (TC:660-712), `Activity` (TC:796), `Finished (N)` (TC:747), and on each line `Move X to its own tab`, **`Open X beside its session`**, **`Send X back into this tab`** (TC:983-1001), Brief and Answer (TC:1050, 1070). The last two moves differ in words from the catalogue's.
- **Tab strip**: `×` is `tab.close` (CST:463, SM:381), `+` is `chat.new`, fresh mark is `tab.fresh`, pin mark is drawn only. Keys: F2 renames, ⌫ closes (registry 620, 632; CST:359-360), ⌘⇧S Use skill (549), `shell.new` (537).
- **Smart close** has no row anywhere. Its only way in is the third button of End's dialog (EndingChat.tsx:173-181).
- **Drag only:** reordering tabs has no menu or palette row (CST:177-197). It takes the pointer or the keyboard sensor (sortable.tsx:6, 63). A split's divider is moved only at the divider (LP:797).

## What I couldn't determine

- **Nothing was run.** Every label computed by `unnamed()` and every condition is read, not seen. Which rows appear depends on runtime state: running, stopping, restoring, queue, policy, Skill studio flag, read-only project (`readOnlyOffers` A:3323 turns rows off).
- **Row 19, `Show` on a sidebar row.** `menuOn` lists it (A:5031), but the Chats section is handed `found` (PV:9500), built from the catalogue called with `needsYou: NO_QUEUE` and no `stopped` (PV:7736-7891), and a missing id is dropped (A:5305-5311). So by my reading it never draws. The queue rows reach only the report the palette reads (PV:8659). I classed it palette-only on that reading. A running app would confirm it.
- **Row 10 on a breadcrumb.** `chat.restart` exists only for `!chat.tab` (A:4182). The binding says a task has no tab "until the person opens it" (bindings.ts:8191-8192), while the comment at A:4173 expects a task shown inside its session's tab to have the row. Whether showing a task in that tab sets `tab` is core behaviour I did not trace. If it does, the row goes from the breadcrumb and from the sidebar for that task.
- **Which pane a [ctx] row acts on.** `Menued on="pane"` runs rows through `press` (PV:9859), on the focused pane, where [btn], [crumb] and [term] use `onPaneDoes(layout.pane, …)` (LP:736, 770, 928), which `ChatStrip` says focuses that pane first (CST:540-542). Whether a right-click on another pane's header moves focus first was not settled; `PaneFrame` sets no pointer handler for it (LP:189-201).
- **Whether a right-click on a pane's chips reaches the panes' menu.** Comments say so (TM:27-30), and the chips have no context menu of their own, but it wasn't exercised.
- **Row 9** assumes the header chip's `glance.label` and the palette's `card.label` are the same words. Both come from the harness card; I did not trace the two sources to one field.
- **The 88** is a hand count of families and moves with how they are grouped.
- **Rows 53-56** were not checked for what `now.browsers` holds for each case beyond the branches shown.
- **Native app menu** (Tauri) was not read. A comment mentions its Settings… (A:2422); it may hold other rows.
