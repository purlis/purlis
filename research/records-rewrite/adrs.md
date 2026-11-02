# ADRs that still state or rely on "a chat is a tab" (for #4704)

Read at `research-records-rewrite` (origin/main 421410a1b). Paths are under `docs/adr/`.
Amendments are separate files in `docs/adr/<NNNN>/` (#1982 rule), so each new amendment here is
a new file in that folder. Ruling refs: D1–D9 = #4576; E1–E8 = #4588's eight items;
S1–S4 = #4587 "asked and ruled"; P1–P6 = #4585; T1–T5 = #4582; G1–G4 = #4583; W1–W4 = #4701.

## A. Need an amendment (main list)

### 0033 A plane is a project, and a window may hold several (known; confirmed)
- `0033/2026-09-26-splitting-a-project-into-a-window-of-its-own.md:15-19` "One project is held by one window ... Opening a project another window holds brings that window to the front".
- same file `:53-54` "A project moved to another window comes back as tabs without the splits it had: the split layout of a project's panes is the window's state and is not carried across." Now wrong: tab trees and shares are in `reopen.json` (S1, #4587 "decided": `tabs` list).
- Overturned by W1–W4 (#4701), S1.
- Gist: a project may also have **tab windows** (strip and panes, no sidebar, no project strip) that belong to its one project window; closing one sends its tabs back behind the front tab and ends nothing; a press goes to the chat's pane in any of the project's windows, else the most recently used pane across them. Drop the "comes back without its splits" cost: a tab's tree, shares and window are recorded.

### 0034 purlis keeps a little state outside every plane (known; confirmed)
- `0034-charter-keeps-a-little-state-outside-every-plane.md:42` "how they were arranged in windows at the last quit".
- `:58-59` "A chat pin is **not** here ... that one goes in the plane's own `.charter/app/reopen.json`."
- Overturned by W4 (tab-window places), #4700 (tab pin).
- Gist: "arranged in windows" now includes each tab window's place on screen, keyed by project and tab-window number (which tab is in which window stays in `reopen.json`). Reword the 0040 note: the chat pin is retired; a **tab** pin lives in `reopen.json`'s `tabs[].pinned`.

### 0036 The workspace is an axis again (known; more lines found)
- `0036-...md:69-74` "Which workspace a chat is in is the plane's answer ... It is deliberately **not** a field recorded on the tab". Overturned by S1: every tab records its strip; the chat is still filed by the plane's answer. (Amendment argument per S1: a tab's strip is a fact the person made, not a copy of the plane's answer.)
- `:78-81` "Focusing a workspace brings one of ITS chats to the front — the one that was in front there last — and a workspace with no chats puts nothing in front. Leaving another workspace's chat on screen under an empty strip would be the app drawing a chat the strip says is not there." Overturned by S2 (each strip's front **tab** is recorded; first launch shows first chat in old order; empty pane only with no chats), E1 (empty pane), and G2 (a drag may put another workspace's chat in a pane, pane head names its workspace).
- `:86-89` "The palette's `Switch to tab` rows and the needs-you queue both show a chat by bringing its tab to the front". The invariant (front tab is on the drawn strip) still holds (E3), but the mechanism is now D1/E3: replace the focused pane of that workspace, or jump. P "Gone": `Switch to` leaves the tab menu.
- `:144-152` "The words say that a close ends the chat ... The row is now `End chat 3 steward` ... The pane close and the project close say the same". Overturned by D2/P4: closing a pane or tab never ends a chat; End… is a separate chat action; the #3295 danger cue goes.
- `0036/2026-10-12-the-chat-strip-is-the-editor-s-tab-bar.md:18-19` "the chat strip still shows the focused workspace's chats"; `:27-29` "drawn in all three of the centre's states: the panes, 'No chats in this workspace' and 'No workspaces yet'"; `:41-46` "a tab gives up its task chip before its title". Overturned by T1 (Name tabs: a tab shows its focused pane's icon and name, `⊟+N`), T2 (one empty pane replaces the three empty states), and P "Gone" (this tab's tasks go to the Keyboard fog; #4699: "The focused chat's tasks").
- Gist: a tab records its strip (workspace or `None`); the strip is a strip of tabs (layouts), not of the workspace's chats; closing never ends; a press brings the chat's workspace forward and replaces that workspace's focused pane; a drag may show another workspace's chat; drop the "End chat" wording rule.

### 0039 Tabs keep their order, and the overflow sorts by activity (known; more lines found)
- `0039-...md:12` "Pinning at all three levels — project, workspace and chat."; `:74` "a project, a workspace and a chat can each be pinned"; `:94-97` "A chat ... A chat pin is therefore an app record"; `:125-126` "If it is not exempt, pinning a chat buys only a marker"; `:156-158` "A pinned chat outlives the thing it pins, or it does not ... A pin has to disappear with it."
- `0039/2026-09-22-the-row-does-not-scroll-and-the-three-strips.md:109-111` "a pin draws a tab first, so it is the last thing to go, and no exemption of its own" (replaced by #4702).
- `0039/2026-09-26-the-operator-can-drag-a-tab-and-the-strip-still.md:23-24` "A tab stays on its own strip. Nothing is dragged between strips, or out of the window into a split window of its own." Overturned by W3 (drag a tab off the window makes a tab window) and G3 (drag a tab down into a split). `:37-41` "Chats and view tabs: ... `reopen.json`, whose `chats[]` are now written in the strip's order ... the file a chat's pin is already in". Now the `tabs` list holds strip order (S-decided).
- `0039/2026-09-28-a-tab-being-smart-closed-is-drawn-first-and-moved.md` (whole file: the smart-close chip before the pinned tabs). Retired by P5: the strip chip goes; wrapping up is a breathing mark on pane head and row.
- Overturned by #4700, #4702, P5, W3, G3.
- Gist: pins at three levels, project, workspace and **tab**; the chat pin retires; a pinned tab keeps its place first and survives relaunch, upgrade and cleared panes but not its contents; a pinned tab is exempt from the collapse (icon-only past half the strip), other tabs share down to a 120px floor. The smart-close chip amendment is withdrawn. A tab can leave its strip by a drag into a split or off the window.

### 0040 A pin is an arrangement, so it is machine state (not yet known)
- `0040-a-pin-is-an-arrangement-so-it-is-machine-state.md:4` "a project, a workspace and a chat can each be **pinned**"; `:42-54` "**A chat pin goes in the plane's own app record** ... A pin on a chat has to disappear when the chat does"; `:130-132`, `:137-139` (rejected list, chat pin); `:155-157` "Whether a pinned tab is drawn first, and whether it is exempt from the overflow menu, are ADR 0039's open questions".
- `0040/2026-09-26-a-workspace-pin-keeps-the-order-it-was-pinned.md:28-30` "Project pins and chat pins are unchanged too: each is a flag on its own entry".
- Overturned by #4700 (pin moves from `Chat::pinned` to `tabs[].pinned`), #4702 (exemption settled).
- Gist: the third level is a **tab** pin, kept in `reopen.json`'s `tabs[]` (still per plane, out of git, so the machine-state ruling holds). The reason changes: a tab pin does **not** disappear with its contents; it disappears when its tab is closed or its workspace removed (S1). The "absence reads as false, no version bump" argument carries to the new `tabs` list (S: "a record with no `tabs` list is read the old way exactly once"). The open questions at `:155-157` are now answered by 0039's amendment.

### 0043 A panel is a declared contribution (known; confirmed)
- `0043/2026-09-23-a-tab-holds-a-session-or-a-view.md:21-27` "A tab is a layout of panes, and a pane holds a session or a view ... a tab holding only a view has ... a close that ends nothing and asks nothing" (now every close ends nothing; a pane may also be **empty**).
- `:40-45` "An approved extension that offers a view gets a tab when the operator opens it ... nothing it declares says *where*" (stands; add Open beside).
- `:47-48` "View tabs are in the plane's reopen record beside the chats" (now nodes of a tab tree in `tabs`).
- `:56-59` open question "whether a view may be split beside a chat by anything but starting a chat beside it" (answered by #4703).
- Overturned/answered by S-decided (empty pane, binary tree), D2, #4703, #4705, #4699 addendum, #4702.
- Gist: a tab is a binary tree of panes; a pane holds a chat, a view, or nothing. A view never replaces a pane's content; a press opens it in its own tab, or with **Open beside** (⌥-press / ⌥Enter) in a split beside the pane pressed from. Without a press an extension can only **offer** its view (pane band and Notices centre note; Dismiss quiets that view and subject until relaunch; "Stop offers from <extension>" in Settings › Extensions; a removed extension's offers are withdrawn and its open view says "not here any more"). A split that can't give both sides about 40×8 folds the unfocused side to a rail when drawn; tree and shares unchanged.

### 0054 Projects live in the title bar, and the workspace strip draws pins (not yet known)
- `0054-projects-live-in-the-title-bar-and-the-workspace-strip-draws-pins.md:100-106` "## The chat strip does not change ... The strip keeps ADR 0039 as amended: fixed order, pinned first, width-fill, show-more by activity ... chats are the work in front of you."
- Overturned by D3/T1 and #4700: there is no chat strip; the strip holds tabs, a tab is pinned, not a chat.
- Gist: one short amendment: the centre strip is a strip of tabs (layouts), pinned tabs first; chats are in the sidebar. (Title is about workspace pins and was already overtaken by the 2026-10-23/24 amendments; nothing new to supersede.)

### 0062 A harness started by hand in a shell tab is warned about (title wrong; not yet known)
- `0062-...md:1` title "in a shell tab"; `:3` "SI-5 (plain shell tabs)"; `:5` "A **shell tab** is the operator's own `$SHELL` in a tab of the app"; `:7` "`New shell`, beside `New tab`"; `:10-11` "is filed on that workspace's strip, is recorded and put back like any chat, and its tab wears a terminal's mark"; `:24` "a chat tab is where it should run"; `:27` "The window draws a banner on that tab"; `:100` "A new CONTEXT.md term: **Shell tab**".
- Overturned by E7 (a shell follows chat rules: New shell takes the focused pane, closing or replacing its pane leaves it running in the sidebar, End is separate, its row goes and its pane clears when its program exits, a pin works as a chat's), P3 (shell's action list), #4588 hand-on ("the shell tab entry").
- Gist: the term becomes **shell**; it has a sidebar row, takes the focused pane, and the started-by-hand banner is drawn on its pane band. The detection decision (shims on the shell's `PATH`, `StartedByHand`) is unchanged. Recommend an **amendment**, not a superseding ADR: the decision is about the shim, and only the noun in the title is stale.

### 0064 Smart close sends its prompt, and the record command closes the tab (known; title wrong)
- `0064-...md:1` title; `:30` "the name the chat's tab shows"; `:92-93` "the command says the tab will not close by itself"; `:112-113` "the command that writes the record is the command that closes the tab"; `:122-129` "the app closes that tab as Close does ... the tab goes back to normal ... The tab wears a closing look ... Its menu offers **Cancel smart close**"; `:133-134` "**Asking.** Closing a chat asks **Smart close** (primary), **Close** or **Cancel**"; `:285-302` "The tab becomes a chip ... before the pinned tabs ... `tabs.frontWithout` ... A tab with a second chat beside it in a split does not become a chip"; `:306-318` "the chip goes ... the tab comes back **in its old place**"; `:228`, `:343`, `:356` (tab closes on the line).
- `0064/2026-10-06-the-record-is-a-brokered-write-and-a-person-s-act.md:15-19` "The tab closes on a record only while a person's smart close is under way: the tab's **Smart close** ... a record the app wrote closes the tab".
- Overturned by P5, D2, S3/#4587 item 3.
- Gist: Smart close is the third answer of End…'s dialog, wherever End… is (pane head ⋯, row ⋯, palette); its strip chip goes; wrapping up is a breathing mark on pane head and row, with Cancel smart close as the head's state button and first in the list. When the record is written the chat **ends** like End: its pane keeps the last screen with Resume and Clear pane and links the record. A stop with no record goes to the pane band, bell and needs-you row. Closing a pane asks nothing. Title: recommend an **amendment** that says in its first line "the record command ends the chat (it no longer closes a tab)"; the decision (the click is consent, so the prompt is sent; completion is the command's signal) stands, and superseding would orphan 22 tool-allow amendments that live under 0064.

### 0067 A chat runs in a sandbox (not yet known; substantive)
- `0067-...md:29-30` "The operator can turn it off for one chat, and that chat's tab shows it for as long as the chat lives."; `:545-546` "**The tab shows it for the chat's whole life** (SD-3): every tab carries a sandbox badge, and an unsandboxed chat's badge cannot be hidden or dismissed."; `:535-536` "The tab badge names the backend."; `:85`, `:92` "allows its own on its tab", "the tab offers Restart now".
- `0067/2026-10-09-03-one-chat-s-own-allow-is-bound-the-same-way.md:15-17` "The tab's chip keeps one hand for each chat waiting off screen. Its menu lists chats ... the question for several tasks is a Notice on the tab".
- Overturned by D3/D4 (a chat may be in no tab), E5 (the red count on a tab moves to the row), T1 (a tab shows its focused pane's icon only).
- Gist: SD-3's always-visible opt-out badge moves to where every open chat always is: its **sidebar row** and, when shown, its **pane head**; a paneless chat's block asks show on its row, the ✋, the chip count and the Inbox (E5). The tab's hand-chip for chats in the tab goes. Security-visible: "cannot be hidden" has to be re-proven for a chat with no pane (the row is under the Workspace filter; All lists it).

### 0068 A chat lives in charterd (amendments only; not yet known)
- `0068/2026-10-14-amended-for-2012-a-browser-is-a-hosted-program.md:49-50` "a browser lives as long as a tab shows it: when the last such tab closes it ends" (a person's browser view).
- `0068/2026-10-16-amended-for-2006-chats-survive-a-crash-and-reattach.md:57`, `:64-65` "keeps everything of it as for a start (its tab, ...)" and "What no chat put back reattached is closed ... so nothing runs with no tab".
- `0068/2026-10-26-amended-for-2025-browsers-survive-a-crash.md:49-53` "comes back in a view tab ... A chat's comes back on its row and its chat's tab as well."
- `0068-...md:388` "a shell tab is not a harness and has no resume".
- Overturned by D3/D4 (a chat can run with no tab), E7.
- Gist: reword "nothing runs with no tab" to "nothing runs that the reopen record does not list" (the rule itself stands). A browser view's lifetime under "closing never ends" is **not ruled** (see conflicts).

### 0072 purlis has five concepts (not yet known)
- `0072-...md:60` "**Chat** | One conversation with an agent, in a tab, in one workspace or at the project root."
- `:122-123` first-hour surfaces "the workspace and chat strips, the new-chat picker, a chat's tab and header".
- Overturned by D1–D3, T1.
- Gist: a chat is a conversation listed in the sidebar and shown in a pane when the person picks it; the first-hour surfaces become the tab strip, the sidebar row and the pane head. (`:58` "the tab the app shows it in" for Project is stale from 0054's rail amendment, not from this map; fix in the same pass.)

### 0073 A harness is declared as data, at one of three levels (not yet known; conflict)
- `0073-...md:108-112` "**A chat in a tab starts in its terminal (V24a)** ... A chat with no terminal shown (dispatched, headless, remote) starts at the highest level its harness offers"; `0073/2026-09-30-ruled-v24.md:3`.
- Affected by D4: handoffs now start sidebar-only, so by this text they would start at level 3, where before they opened in a tab at level 2.
- Gist: re-key V24a off "in a tab" (e.g. "a chat the person starts or follows starts in its terminal; a task starts at the highest level"). Needs a call, see conflicts.

### 0076 A run moves only by a named cause, and a chat's state is read from its runs (not yet known; substantive)
- `0076-...md:97` "**A shell tab has no run**"; `:139` "`stopped | closed` — the operator closed the chat: its tab (Smart close included, ADR 0064)"; `:140` "the operator stopped this run and kept its tab (Q11)"; `:287` "**open** | it has a tab."; `:289-290` "closed with a record | its tab closed after `purlis session record` ..."; "closed without a record | its tab closed any other way".
- Overturned by D2 (closing a pane only clears it; ending is separate), D4 (open chats with no pane), S3 (ended harness chat keeps its pane read-only with Resume / Clear pane).
- Gist: **open** means "not ended", with or without a pane; `closed` is caused by End… or Smart close (or Start fresh), never by closing a pane or tab; "kept its tab" becomes "kept it open". A shell has no run.

### 0080 A level-3 chat is an ACP client session (not yet known)
- `0080-...md:17`, `:67` "A tab chat starts in its terminal"; `0080/2026-10-01-ruled-v28.md:5-7` "A level-3 chat opened in a tab shows its transcript as a view tab ... New tab chats still start in their terminal"; `0080-...md:306-309` "A level-3 chat has no terminal tab to show that opt-out on ... offers it at its next level in a tab, where the opt-out is visible".
- Overturned by D1/D3, and tied to 0067's badge move.
- Gist: a level-3 chat shown in a pane draws its transcript in that pane (not a separate view tab); the Windows refusal's reason moves from "no terminal tab" to the badge's new home.

### 0081 purlis is where agents work; writing code by hand opens in your editor (not yet known)
- `0081-...md:63-64` "a shell tab's shell (ADR 0062). The window's panes split to hold tabs side by side" (inverted: tabs hold panes); `:127` "a shell tab; split panes that hold tabs"; `:182` "`reveal` | asks the window to bring forward a chat's tab"; `:194-196` "It brings the window and the tab forward with focus on the tab in its strip"; `:339-340` "the tab comes forward without terminal focus"; `:342` "a level-3 chat's view tab"; `:398` "`reveal` brings the tab forward with focus off the terminal"; `:135`, `:423` "a shell tab".
- Overturned by D1, E1–E3, W2, S-decided (a tab is a tree of panes), E7.
- Gist: tabs hold panes; `reveal` shows the chat by the press rules (jump if on screen in any of the project's windows, else the focused / most recently used pane, workspace brought forward), with focus on the pane head, never the terminal. Whether an editor's `reveal` counts as the person's press is not ruled (see conflicts).

### 0084 A review is a comparison git computes (not yet known)
- `0084-...md:11` R1 "a Changes button on every chat tab"; `:62` V34a "The chat-tab button is **Review**"; `:198` "**The chat tab's button is *Review***"; `0084/2026-10-01-ruled-v34.md:3-5` "The chat tab's button is *Review* ... no button on the chat tab".
- Overturned by D9/P1/P2 (chat actions live on the chat: pane head, row ⋯, palette; one list).
- Gist: Review is a row of the chat's one list (and, if the prototype shows it directly, on the pane head), not a tab button. R2's Review **view** tab, splittable beside its chat, stays right (now via Open beside).

### 0090 Agents work together as chats (not yet known; substantive)
- `0090-...md:55-57` "the app opens a handed-off chat only as a tab on a strip"; `:82` "whether or not it has a tab"; `:89` "A tab is one way to be listed"; `:97` handoff opens "a tab on the target workspace's strip"; `:242-243` "**A headless chat is a chat with no tab** ... the operator can open its tab at any time"; `:246-250` "Until AC-12 ... a dispatched chat opens as a tab ... A dispatch that would start a level-1 harness headless opens it as a tab instead, and says so."; `:320-324` "and so on a tab on a strip ... Until AC-12, a handed-off chat still opens on a strip."
- `0090/2026-10-07-amended-and-accepted.md:42-48` "headless means 'has no tab yet' ... Pressing it shows it inside the tab of the session that asked for it, and adds no tab ... A handoff opens as a tab, as before."; `:52-53` "Closing an asking chat asks once what to do with the chats it started"; `:54` "rolls up to ... the workspace's tab"; `:93`, `:139` "in the persona chat's own tab".
- `0090/2026-10-08-a-task-ends-at-its-report-and-who-may-dispatch.md:24` "Reopen resumes ... as an ordinary chat with a tab"; `:36-37` "While the task is the chat in front, the end is held"; `:92-94` "The cross and the close shortcut on a task's own tab send the task back to the Chats list".
- `0090/2026-10-14-a-task-always-reports-back.md:38`, `:56`, `:76` "the task's tab".
- Overturned by D4, E4 (a task press replaces the focused pane, not "inside the asking tab"), E5, E6 (handoff: chip card, Inbox Update, row "new" mark; carries handoff.md's safety property), S4 (`looked_at`), D2.
- Gist: every dispatch, handoff included, starts in the sidebar and takes no pane; a handoff is announced by the chip card, an Inbox Update and the row's "new" mark, which is the new form of "visibility is the control". "Headless" = "in no pane". A pressed task lands like any chat. The end is held while the task is in `looked_at` (any pane of a front tab on screen). "Closing an asking chat" becomes "ending". Level-1 exception: see conflicts.

### 0096 Your sandbox level is yours (not yet known; small)
- `0096-...md:85` "The title bar shows the level of the chat in front, as its tab does."
- Gist: "as its pane head and row do"; "the chat in front" = the focused pane's chat. Same move as 0067.

### 0102 A skill router decides what a chat sees (not yet known)
- `0102-...md:39` "shown on the chat tab"; `:85` "The chat tab and the studio show the router's timings".
- `0102/2026-10-21-the-skill-budget-and-use-skill-as-built.md:15` "on its tab's Use skill… row"; `:26` "a row on each chat tab's menu ... the tab's row then says the budget"; `0102/2026-10-30-...md:10` "each chat tab's menu row"; `:31` "in a split tab, on the pane that has the keyboard"; `0102/2026-11-01-...md:3`, `:13` "The chat's tab".
- Overturned by P1/P2/P4 (tab menu holds tab things only; Use skill… is on the chat's one list; the skills chip becomes a row of the list).
- Gist: Use skill… lives on the chat's list (pane head ⋯, row ⋯, palette) and P1 moves the Skills chip off the pane head into the list; "shown on the chat tab" → on the chat's Skills view / pane.

### 0061 A curation action is a chat with its prompt typed (small; not yet known)
- `0061-...md:126-128` "its tab says `<label> · <subject>` ... and on the plane root's tab otherwise".
- Gist: the chat the person starts by a Curate press takes the focused pane (D4's `+` rule) and its **name** says `<label> · <subject>`. (The "Curate ▸" on workspace/persona/plane-root menus is not a chat-tab menu and stays.)

### 0095 Every sign-in belongs to a profile (not yet known; conflict)
- `0095-...md:239-244` "**The Ask's action is a shell tab** ... Sign in opens a shell tab at the project's root ... seen in that tab"; `0095/2026-10-27-a-profile-is-kept-and-signed-in-on-each-runner.md:30`, `:35`; `0095/2026-10-31-a-sign-ins-ask-ends-when-signed-in-or-when-you-leave-the-shell.md:7-8` "every shell tab that sign-in was typed into has closed or its program has ended".
- Overturned by E7/E8.
- Gist: Sign in opens as a **one-job** split beside the focused pane (E8 names "a forge login typed into a shell"), which goes when its program exits; the 10-31 end condition becomes "its program has ended". Needs a call, see conflicts.

### 0038 The window is four regions (amendments; partly known)
- `0038/2026-10-07-the-left-region-also-lists-the-project-s-chats.md:13-15` "pressing it brings that chat's tab forward on whichever workspace's strip it is on. A chat started as a task has no tab until its row is pressed".
- `0038/2026-10-10-03-three-scopes-in-place-of-the-chip.md:3-11` "switches between **This tab**, **Workspace** and **All** ... **This tab** lists the chats of the tab in front"; `:25` "outside this tab".
- `0038/2026-10-10-04-explorer-is-in-sections.md:7-8` "its menu is the tab's" (workspace rows; workspace "tab" is now a chip, fine) — no change needed.
- `0038-...md:123` "The chat's own tab and the pane's corner are the two places suggested" (unruled suggestion; reword to pane head).
- Overturned by D1, D4, D5.
- Gist: a press replaces the focused pane (jump if on screen; another workspace comes forward); no chat gets a tab from a row press; the This tab scope goes, Workspace and All stay. The four-region decision itself stands.

## B. Wording only (no decision changes): "the chat's tab" meaning "where that chat is drawn"

These say a Notice, a mark or an offer is shown "on the chat's tab". Under the map that is the chat's **pane band** when shown, and its **row / ✋ / Inbox** when not (E5, #4584). One sweep, or a sentence in 0067's and 0090's amendments, covers them; none needs its own amendment.
- 0064/2026-10-07-02 `:16`, `:21`; 0064/2026-10-09-01 `:13`; 0064/2026-10-09-02 `:4`, `:26`, `:41` ("the asking session's tab wears the hand" is the E5 move: hand goes on the row); 0090/2026-10-09 `:28`, `:67`; 0090/2026-10-30-messages `:53` ("a task the person started from a tab" = started by the person).
- 0067 amendments: 2026-10-06 `:9`, 2026-10-09-01 `:8`, 2026-10-13-01 `:13`, 2026-10-14-05 `:21`, 2026-10-14-14 `:47`, 2026-10-14-16 `:34`, 2026-10-15-02 `:38-40` (Open … "its own tab" is a view, fine), 2026-10-15-05 `:25`, `:49`, 2026-10-16-a-project `:19`, `:32`, 2026-10-16-purlis-s-own-wrap `:46`, 2026-11-01 `:10`.
- 0068 amendments 2510 `:23`, 2590 `:20`, 2013 `:34`, 2207 `:29`; 0078 `:142`, `:364`, 0078/2026-10-20-06 `:18`, 2026-10-27 `:57`, 2026-10-29 `:23-24`, 2026-10-30-07 `:15-30`, 2026-10-30 (two files) `:15-34`, `:25`, `:35`, 2026-11-01 `:35` ("What a restarted window finds and no tab takes back is closed": reword to "no record puts back").
- 0099 amendments 2026-10-21 `:30-32`, 2026-10-22 (title "said on the chat's tab") `:1`, `:8`, 2026-10-24 `:28-36`, `:65`, 2026-10-26 `:15`, 2026-10-27 `:7`, 2026-10-22-draft `:21`; 0063/2026-10-22 `:30`; 0093/2026-10-15 `:4` (history).
- Shell tab as a noun: 0069 `:33`, 0071 `:44`, 0073 `:154`, `:206`, 0074 `:78`, `:209`, 0085 `:20`, 0087 `:247`, `:346`, `:412`, 0089 `:399`, 0080 `:27`, `:171-172`, `:191`, 0067/2026-10-03 `:12`, 0034/2026-09-30 `:13`, 0027/2026-10-30 `:41`, 0038/2026-10-11-03 `:35` ("shell tabs stay tabs (ADR 0062)" — becomes "shells are chats' kind of terminal, in panes").

## C. Checked, mention tabs, and stay right
- **0065** memory view tab: views open in their own (preview) tab; `:146-148` already keeps a preview off "a tab split beside a chat". Fits #4703. Title right.
- **0043 main body** (`:353-374`): persona/memory views in tabs; right.
- **0047**, **0051**, **0060**, **0089** §12 (Runners/Devices **view** tab), **0101**, **0105/2026-10-28** (scorecard tab, read-only chat transcript as a view tab): all view tabs.
- **0048**, **0050**, **0056**, **0058**, **0096 `:20`**: "settings tab" is a view. (0048 `:229-230` workspace tab tint is stale from 0054's title-bar move, not from this map.)
- **0105** `:105-107`, `:177`: a grade's chat "has no tab, is listed nowhere": still true (no pane, no row).
- **0037**, **0094**, **0103**, **0104**: the tabs widget/family and a11y (roving tabindex, `Tab` key); the strip is still a tablist.
- **0026**, **0082**, **0086** `:45`, `:179`: measurements ("tab or pane switch"). Still right; consider adding the sidebar pick to L2 once #4692 measures it.
- **0033 main** `:46-65`, **0054** amendments (project rail, plane root first tab, workspace chips), **0052/2026-09-26**, **0041/2026-09-23**: project/workspace tabs or split windows, not chat tabs.
- **0066** `:21`: history of the word "chat".
- **0106** `:54`: the Done tab of a view. **0079** `:101`, `:222`: memory tab = view. **0044** `:28`: names the view-tab amendment.
- **0102** `:29-69`, **0095** `:291-303` "pin": skill pins and harness version pins, unrelated to tab pins.

## D. Conflicts and open points the new wording would hit
1. **0090 §7 level-1 exception vs D4.** 0090 `:248-250`: a level-1 chat can't say it is waiting, so a dispatch that would start one headless "opens it as a tab instead". D4 says tasks and handoffs never take a pane. Either D4 gets this exception (a level-1 dispatch takes a pane, or a new tab) or level-1 dispatch is refused. Recommend: refuse a level-1 **task** and start a level-1 **handoff** in the sidebar with E6's three announcements plus a row mark "can't say when it waits". This is a ruling to make, not a rewording.
2. **0073/0080 V24a keys the start level on "in a tab".** With handoffs paneless (D4), 0073 `:110-112` would start them at level 3 (transcript, no terminal), a silent behaviour change for every handoff. Recommend re-keying on "a chat the person starts or follows (the person's chat, handoff) starts in its terminal; a task starts at the highest level". Needs a ruling.
3. **0067 SD-3 "every tab carries a sandbox badge ... cannot be hidden".** A chat in no pane has no tab. Moving the badge to the row and pane head works, but the row can be filtered out (Workspace scope), so "cannot be hidden" needs restating (for example, the row in All and the ✋ list). Security-visible; check with the security review's rules before wording.
4. **0095 harness sign-in: E7 vs E8.** "Sign in" opens a shell and types the sign-in. If that is a shell (E7), its pane close leaves it running and 0095/2026-10-31's "has closed" end condition no longer fires; if it is a one-job program (E8, which names "a forge login typed into a shell"), it opens beside and goes on exit. Recommend E8, and the 10-31 condition becomes "its program has ended".
5. **0081 `reveal` from an editor.** Not covered: it isn't the person's press in the window, but it isn't D4's "started without the person" either. Recommend treating it as a press (D1/E1–E3/W2) with focus on the pane head, as ED-2 already keeps focus off the terminal.
6. **0068/2012 the person's browser ends "when the last such tab closes".** Views follow no "close never ends" ruling: D2 is about chats and E7 about shells. Whether closing a browser view's pane ends the browser is unruled; worth a line in the 0043 or 0068 amendment.
7. **0064 "Smart close (primary)" vs P5 "the third answer of End…'s dialog".** The 0064 default/primary may or may not survive; P5 only places it. Say which answer has the keyboard.
8. **0036 `:79-81` vs G2.** "Leaving another workspace's chat on screen under an empty strip would be the app drawing a chat the strip says is not there" is exactly what G2's drag now allows (badge on the pane head). The 0036 amendment should cite G2 as narrowing E3 to presses.
9. **0036/2026-10-12 tab task chip.** T1 defines what a tab shows and says nothing of the per-tab task chip; P "Gone" sends "this tab's tasks" to the Keyboard fog, and #4699 makes it "the focused chat's tasks". The amendment can drop the chip, but nothing rules where a chat's still-working task count is drawn (pane head or row).
10. **0040's reason flips.** 0040 put the chat pin in `reopen.json` *because* a pin must vanish with its chat; #4700's tab pin explicitly outlives its contents. The conclusion (same file) stands, but the amendment has to give a new reason (a tab exists only in that record, and a tab number means nothing outside the plane), not repeat the old one.
