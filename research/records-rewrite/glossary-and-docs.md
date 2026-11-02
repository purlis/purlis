# Records rewrite for "a chat isn't a tab": glossary, docs, handoff.md, site (#4704)

Read at `421410a1b` (origin/main) in `.worktrees/purlis/research-records-rewrite`. Nothing was
edited, run or built. Rulings are cited as in `rulings.md`: D1 to D9 (#4576), and #4582, #4583,
#4584, #4585, #4587, #4588, #4699, #4700, #4701, #4702, #4703.

Line numbers are today's. Several differ from the brief's (CONTEXT.md's Task is at :853, not
:849; handoff.md's "opens as a tab" is :1107, not :1096).

**Skipped on purpose:** view tabs (Settings, a persona's, a vault's, Review, Changes, Activity,
Dispatches, a memory's, a skill's, file tabs), browser tabs, the project and workspace strips,
the plane root's tab, the keyboard's Tab key, `tabIndex`, a tab character, "table" and "stable".
ADRs and `docs/agents` were out of scope.

## Counts

| Group | Items that change | Of which new entries or full rewrites | Gaps or contradictions marked |
|---|---|---|---|
| (a) CONTEXT.md | 34 | 5 new entries (Tab, Pane, Empty pane, One-job pane, Tab window), 8 rewritten in full, 1 retired (Headless chat) | 9 |
| (b) docs/ (spec, ui-primitives, ui-copy, design-system, plane-format, getting-started, updating, uninstall) and other crate docs | 79 numbered (b12 needs no change, b13 is a question only) | 5 full rewrites (spec terminal editor, spec Smart close, ui-primitives pane rows and band, task-keys section, reopen.json `tabs[]`) | 19 |
| (c) handoff.md | 40 (the rewritten section plus 39 line items) | 1 full section (:91 to :123) | 4 |
| (d) site and READMEs | 2 (site index, README) | 0 | 1 |

No change needed: `docs/design/foundation/README.md` (22 hits, all file tabs and the workspace
tab), `docs/component-stories.md` (Tab key), `docs/extension-api.md` (a skill's view tab),
`docs/cluster-operator.md`, `crates/purlis-core/docs/workspaces.md` (a tab character),
`AGENTS.md` and `CLAUDE.md` at the root (no hits), `CONTRIBUTING.md:222` ("a tab moved" still
holds). `app/` has no AGENTS.md, CLAUDE.md or README.md.

**Marks used below.** ⚠ CONTRADICTS: today's text, or a natural rewording, breaks a ruling.
❓ NOT RULED: the rulings don't say enough to word it, and the wording given is a recommendation.

---

## (a) Glossary entries in CONTEXT.md

There is no **Tab**, **Pane**, **Empty pane**, **Tab window**, **One-job pane**, **Front chat**
or **Close** entry today. The first five are needed; the rest aren't (front chat is internal,
and Close is covered under Pane and Tab).

### a1. Chat, :34-39 (rewrite)

Today: "One of the five concepts: one conversation with an agent, in a tab, in one workspace or
at the plane root."

> **Chat**:
> One of the five concepts: one conversation with an agent, in one workspace or at the plane
> root. It has a row of its own in the sidebar for its whole life, and a **pane** shows it when
> the person picks it. Picking a chat shows it in the focused pane, and the chat it replaces
> keeps running. A chat is in one pane at most. Closing its pane never ends it: **End…** is an
> action on the chat. It is where work happens and what needs you. It has a number the window
> shows (`steward 3`) and an id that never changes, has one or more **runs**, and shows its
> **branches**, one per repo it works in (ADR 0066, ADR 0072).
> _Avoid_: session (that is the process), thread, agent, conversation (that is the harness's),
> chat tab (a chat is not a tab), headless chat (most chats are in no pane; say "in no pane")

### a2. Writing chat, :146 (word swap, with code)

"what _New tab in <repo>_ and _Start new chats in <repo>_ point at" → "what _New chat in
<repo>_ and _Start new chats in <repo>_ point at".
The label is code: `actions.ts:2893` says `New tab in ${repo}` (also `Explorer.tsx:96`,
`ChangesView.tsx:79`). Under D4 the chat takes the focused pane, so "New tab" is wrong.

### a3. Task (of a dispatch), :853-868 (rewrite of the first half; the rest stays)

Today: "is shown inside its session's tab ... Pressing the task switches that tab to it and adds
no tab ... a tab of its own ... a minimise where a session's tab has its close ... a session's
close is the only close on the strip."

> **Task** (of a dispatch):
> A dispatch that expects a **report**, and the chat it starts: work done for the asking chat.
> It starts in the sidebar only, listed under the asking chat, never hidden, and takes no pane
> until the person picks it. Picking a task is picking a chat: it replaces what the focused pane
> shows, or goes to the pane that already shows it. The pane's head then says the path
> (`steward 4 › talk · working`), and pressing a step of the path shows that chat in the same
> pane. Closing its pane ends nothing: the task keeps working in the sidebar. "Task" is the
> word in every sentence the app shows for that chat. *(Unchanged from "Its row says its state
> in a word" to the end.)*

This applies #4588 rule 4, which replaces "pressing the task switches that tab to it".

### a4. Ending a task (by the person), :872-879 (two sentences)

"Closing a task's tab is neither: the tab goes, and the task goes **back to the list** and
keeps working." →

> Closing a task's pane is neither: the pane clears, and the task keeps working in the sidebar.

_Avoid_: "close the task's tab (to mean ending it)" → "close the task's pane (to mean ending it)".

### a5. Handoff, :902-908 (rewrite)

Today: "It opens as a tab. It is never a task: its chat is a session of its own, with its own
tab and a row of its own at the top of the Chats list".

> **Handoff**:
> A dispatch where the work moves: the persona chat takes it from there, and the asking chat
> does not wait on it. Its chat starts in the sidebar and takes no pane and no focus, in this
> workspace or another. Its arrival is said in three places: a card on the target workspace's
> title-bar chip ("<chat> arrived from <asking chat>", with **Go to it**), an **Update** in the
> Inbox, and a **new** mark on its row until the person has looked at it. It is never a task:
> its chat is a session of its own, with a row of its own at the top of the Chats list, under
> no chat and in no chat's counts. Its row says `from <chat>`, and the row of the chat it came
> from says `handed off to <chat>`.
> _Avoid_: transfer, session record (that is what a closing chat writes)

❓ #4588 rule 6 says that with no chip the card is "listed in the Notices centre". The glossary
has no Notices centre: Notices are listed in the **Inbox**. Recommend "listed in the Inbox's
Notices", and the rulings' word changed to match.

### a6. Asking chat, :910-915 (one phrase)

"A task the person asked for themselves from that chat's tab" → "A task the person asked for
themselves on that chat (**Ask <persona>…** in its ⋯ list or the palette)".

### a7. Where you answer, :929-933 (rewrite) ❓ NOT RULED

Today: "**from the top**, in the tab of the nearest chat above that has one, or **from each
session**, in the task's own tab."

> **Where you answer**:
> The person's one remembered setting for where a **task question** that has reached them
> appears: **from the top**, on the nearest chat above it that the person started (its row, and
> its pane when one shows it), or **from each session**, on the task's own row, and its pane
> when one shows it. It is in the Inbox either way. It changes what is shown and never who is
> asked, and any task can still be picked and answered.

"The nearest chat above that has a tab" has no meaning now. The rulings keep the route (#4551)
but don't say what the setting means once tasks have no pane. This wording is the recommendation.

### a8. Reading a task, :951 (word swap)

"The whole conversation stays the person's, in the task's tab." → "…, in the task's pane."

### a9. Quiet note, :955-958 (word swap)

"It goes to the asking chat and to the task's tab" → "It goes to the asking chat and to the
task's row, and to its pane when one shows it".

### a10. Headless chat, :1007-1011 (retire) ⚠ CONTRADICTS

Today: "A chat with no tab yet. It is listed, never hidden ... and it gets a tab when the person
opens it. Never a level-1 chat."

Recommendation: **delete the entry**, and put "headless chat" in Chat's _Avoid_ (a1). A chat in
no pane is now the usual case, so the word marks nothing.

⚠ "Never a level-1 chat" was a guard. A level-1 harness (ADR 0073) has no hooks, so it can't
report an ask, and the person sees one only by looking at its pane. Under D1 and D2 any chat can
leave its pane, including a level-1 one. What partly covers it: `spec.md:70-72`'s faint hand that
names chats which can't report. The rulings don't say how a level-1 chat in no pane is watched.
`docs/adr/0090...md:65` also says "There are no headless chats" (ADR, out of scope here).

### a11. Split window, :1030-1033 (one avoid line)

Unchanged, but add to _Avoid_: "tab window (that holds one project's torn-off tabs)".

### a12. Tab window (new entry, after Split window)

> **Tab window**:
> An OS window that holds tabs torn off one project's strip: the strip and its panes, with no
> sidebar and no project strip. It belongs to the window that holds the project, its **project
> window**, and never holds a project of its own. Closing it sends its tabs back to the project
> window, behind its front tab, and every chat keeps running. Its last tab leaving closes it, and
> it comes back at the next launch (#4701, ADR 0033 as amended).
> _Avoid_: detached tab, pop-out, split window (that holds whole projects)

### a13. Strip, :1048-1052 (rewrite)

Today: "One row of tabs: projects (in the title bar), a project's workspaces, or a workspace's
chats."

> **Strip**:
> One row of tabs: projects (in the title bar), a project's workspaces, or a workspace's tabs.
> A tab on a workspace's strip is a layout of panes; a chat is never a tab of its own. A strip's
> order never changes on its own, and it never scrolls. The operator can drag a tab along it;
> dropped among the pinned tabs it is pinned, and among the others it is unpinned. A chat or a
> pane dropped on the strip becomes a tab. Each workspace's strip, and the plane root's, always
> keeps at least one tab.
> _Avoid_: tab bar, scroller

### a14. Side, :1054-1058 (avoid line)

"_Avoid_: pane (a pane is a chat's terminal), column" → "_Avoid_: pane (a pane is a place in a
tab's layout), column".

### a15. Tab (new entry, after Strip)

> **Tab** (on a workspace's strip):
> A layout of **panes**, kept as a tree: each split holds two parts, side by side or one above
> the other, and each pane holds a chat, a view or nothing. A tab is made only by **Open in new
> tab**, by opening a view, or by dropping a chat, a pane or a tab on the strip. A split stays in
> its tab. A tab knows which strip it is on. Closing it ends no chat: its chats keep running in
> the sidebar. Its menu holds only what acts on the tab: Pin tab, Tile all, Keep open, Close
> other tabs, Close tab (ADR 0043 as amended, #4587, #4585).
> _Avoid_: chat tab, session tab (a chat is not a tab), layout (unqualified), project tab or
> workspace tab (those are other strips' tabs)

### a16. Pane (new entry)

> **Pane**:
> One place in a tab's layout, holding a chat, a view or nothing. Picking a chat (in the
> sidebar, the palette, or with a Go) shows it in the focused pane. A pane holding a view is
> never replaced: the chat goes to the most recently used pane that shows a chat or nothing. A
> chat already in a pane is jumped to, never shown twice. Closing a pane clears it, and its chat
> keeps running in the sidebar; in a split, the split closes. A pane's head says what it shows
> and holds the chat's state button, its ⋯ list, the bell for its notices, Split right, Split
> down and Close pane. When a harness chat's program ends, its pane keeps the last screen under
> an **ended** band with **Resume** and **Clear pane**.
> _Avoid_: tile, frame, terminal (a pane may hold a view), split (that is two panes and the line
> between them)

### a17. Empty pane (new entry)

> **Empty pane**:
> A pane that holds nothing. It says "Nothing in this pane", how many chats are running in the
> workspace, and offers **New chat**; in a split it has its own ×. A strip's last tab keeps one
> when its last pane closes, so a focused pane always exists for `+` and a sidebar pick. In a
> plane with no workspace it says "No workspaces yet" and offers **Create a workspace**.
> _Avoid_: blank tab, empty window, welcome (that is a window with no project)

### a18. One-job pane (new entry) ❓ partly

> **One-job pane**:
> A split that opens beside the focused pane, in the same tab and with focus, for a program that
> does one job and exits: a terminal editor (#1042) or a forge login typed into a shell. When the
> program exits the split goes and the focus comes back. It can't be picked up, closed or
> swapped, and a launch never puts it back.
> _Avoid_: shell (a shell stays until it is ended), popup, scratch tab

❓ #4588 rule 8 names a terminal editor and "a forge login typed into a shell". It doesn't name
the buttons that run a program today: **Install** (a harness's installer), **Sign in to
GitHub/GitLab**, a profile's **Sign in** (`claude auth login`), or `ui-copy.md`'s sudo example.
Recommendation: every program purlis starts for one job is a one-job pane. From the welcome there
is no project and no pane, so a sign-in there needs its own answer.

### a19. Pin, :1089-1095 (rewrite)

Today: "One operator's mark that a project, workspace or chat matters to them."

> **Pin**:
> One operator's mark that a project, workspace or tab matters to them. It is kept on this
> machine and never in the plane. A pinned item is drawn first, and the workspace strip draws
> only pinned workspaces plus the one you are in. A pinned tab keeps its place first on its
> strip, and keeps the tab itself across a relaunch, the upgrade and a cleared pane, but not
> what its panes show. Pinning never makes a tab, and a pinned tab never collapses into
> show-more (ADR 0039 as amended, #4700, #4702).
> _Avoid_: favourite, star, bookmark, pin a chat (pin its tab), pin (for a runner or a browser:
> that is **Pin** (of a runner) or **Pin** (of a browser))

### a20. Show-more, :1104-1107 ❓ NOT RULED

"with the needs-you count of everything it hides". Once the red count leaves the tab (#4588
rule 5), it isn't said whether a hidden tab's chats still count here. Recommendation: keep it,
counted over the chats in the hidden tabs' panes, so nothing on screen hides a wait. Otherwise
delete the clause.

### a21. Needs you, :1109-1117 (one sentence rewritten)

"Every project's are counted on the title bar's ✋, which opens the **Inbox**, and each is
counted in red on its tab and on any show-more hiding it." →

> Each is counted in red on its chat's row. Every project's are counted on the title bar's ✋,
> which opens the **Inbox**, and on the workspace chip. An item never takes a pane or the focus:
> a chat in no pane is answered in the Inbox, and a chat on screen in its pane too.

(Drop "and on any show-more hiding it" unless a20 keeps it.)

### a22. Ask, :1188-1190 and :1192-1194 (rewrite of two sentences)

"A chat's pane draws at most two of its own asks as Notices, from the same list, so an answer in
either place clears both (#1695)." →

> A chat's pane, when one shows it, draws each of its asks as a full card in the band under its
> head. The card stays open while the band is minimised, and Dismiss all skips it. It is drawn
> from the same list, so an answer in either place clears both (#1695, #4584). A chat in no pane
> is answered in the Inbox.

"with Go to chat" stays. Its press now follows the pick rules (a16), so it replaces the focused
pane. That's the person's press, so D4 holds (#4588 rule 5).

### a23. Update, :1202-1211 (add one item)

Add to the list: "a handoff that arrived (with Go to it)". Its sentence "never counted on the ✋"
already holds for it (#4588 rule 6).

### a24. Kill switch, :1213-1219 (one sentence) ❓ partly

"It is a stop, not a close: the tabs stay, each reading as a chat whose program ended." →

> It is a stop, not a close: the tabs and their panes stay, each harness chat's pane showing its
> last screen under the ended band, every chat keeps its row, and each shell's pane clears.

❓ The ended band offers **Resume** (#4587 rule 3). Whether Resume is refused until re-arm isn't
said. Recommendation: refused, with the re-arm sentence as its reason.

### a25. Shell tab, :1221-1226 (rename and rewrite)

> **Shell**:
> A terminal running the operator's own shell, with no harness and no profile, opened by
> **New shell**. It follows a chat's rules: New shell takes the focused pane; replacing it or
> closing its pane leaves it running in the sidebar, so a dev server survives; ending it is
> **End…**, which asks only Cancel or End. When its program exits, its row goes and its pane
> clears. A harness typed into one runs outside purlis's session tracking, so purlis's **shell
> shims** stand first on its `PATH`: the harness still starts, after one line saying so, and the
> shell's pane shows a banner offering to open it as a chat instead (ADR 0062).
> _Avoid_: shell tab, terminal (for the shell), console, plain chat

The shims' name ("shell-tab shims") is also in `plane-format.md:7903`, `uninstall.md:147` and
`spec.md:117-122`. Recommend "shell shims" everywhere; the code's `shellguard` name is fine.

### a26. Review tab, :1253 (word swap)

"as on the chat tab's button" → "as on a chat's Review button".

### a27. Smart close, :1282-1288 (rewrite)

Today: "the tab closes when `purlis session record` tells the app the record is saved ... its tab
says so".

> **Smart close**:
> Ending a chat after it has written its session record: the third answer of **End…**'s dialog,
> wherever End… is. The app sends it one line naming purlis's `purlis-smart-close` skill (the
> operator's click is the consent, the exception to a curation action's never-sent prompt), and
> the chat ends when `purlis session record` tells the app the record is saved, never on
> anything the chat printed. Until then the chat is **wrapping up**: a breathing mark on its pane
> head and its row says so, and **Cancel smart close** or the operator's own typing stops it.
> Once it ends, its pane keeps the last screen with Resume and Clear pane, and links the record.
> _Avoid_: save and close, archive

This applies #4585 rule 5: the strip chip goes.

### a28. Smart-close pass, :1290-1295 (two phrases)

"the tab's **Smart close**" → "**Smart close** in End…'s dialog"; "the only thing that lets a
record close the tab" → "the only thing that lets a record end the chat".

### a29. Plane updated, :1309 (word swap) ❓ placement

"its tab carries a quiet mark saying so" → "its row and its pane head carry a quiet mark saying
so". ❓ #4585's pane head lists icon, name, state mark, "harness · workspace", the state button,
⋯, the bell and the pane controls. It has no slot for this mark, so where it sits is the build's
call.

### a30. Sandbox level, :1509 (word swap) ❓ placement

"shown on its tab" → "shown on its pane head and its row". Same ❓ as a29.

### a31. Block, :1586-1591 (one clause)

"shows a **Notice** on the chat's tab" → "raises an ask, shown on the chat's row, in the Inbox,
and in its pane's band when one shows it". A refused host with Allow is an ask (Ask entry, :1182).

### a32. Skill budget, :1899 (word swap)

"shows it on the chat tab" → "shows it on the chat's Skills row". #4585: the skills chip leaves
the head and becomes a row of the chat's list.

### a33. Use skill…, :1904-1905 (one phrase)

"from the palette, the chat's tab menu or the Skills chip on its pane" → "from the palette or
the chat's ⋯ list, on its pane head or its sidebar row".

### a34. Browser view, :765-768 ❓ NOT RULED

"drawn in a view tab, a tile of the Browsers grid or a chat tab's thumbnail" (and
`ChatStrip.tsx` draws it today). Nothing says where a chat's browser thumbnail goes now.
Recommendation: on the chat's sidebar row, so it's seen for a chat in no pane too. `_Avoid_:
preview (that is a tab's)` still holds.

---

## (b) Docs

### docs/spec.md (13 items)

**b1. :49-51, Language.** "**Chat**: a session as the UI shows it: its tab, its workspace, its
state. **Shell tab**: a chat running the operator's own shell" →

> **Chat**: a session as the UI shows it: its row, the pane that shows it, its workspace, its
> state. **Shell**: a chat running the operator's own shell, with no harness and no profile
> (ADR 0062).

**b2. :58.** "A shell tab draws no state mark" → "A shell draws no state mark".

**b3. :75, Center.** ⚠ CONTRADICTS D6. "**Center:** tabs and free split panes." →

> **Center:** a strip of tabs, each a layout of panes in rows and columns; a pane holds a chat,
> a view or nothing.

**b4. :117-122, shell guard.** "A harness started by hand in a shell tab ... A shell tab — the
operator's own `$SHELL`, no harness, `New shell` beside `New tab` — ... so the tab shows a
banner" → "in a shell", "A shell — the operator's own `$SHELL`, no harness, **New shell** beside
**New chat** —", "so the shell's pane shows a banner". ❓ "beside `New tab`": see b13.

**b5. :170, restore.** "each has its tab in its place from the first frame, drawn as restoring"
→ "each has its row from the first frame, and each tab its panes, drawn as restoring". And "the
chat in front of the project in front first, then each project's chat in front" → "the chats on
screen in the project in front first, then the chats in each project's front tabs".

**b6. :304-309, decision 29.** "drag a tab along its own strip ... chats in the plane's own
`.charter/app/reopen.json`" → "... a workspace's tabs in the plane's own
`.charter/app/reopen.json`".

**b7. :380-384, Smart close.** "and the tab closes only when the record is saved, never on
anything the chat printed; with no record in about five minutes the tab goes back to normal and
stays open." →

> and the chat ends only when the record is saved, never on anything the chat printed; with no
> record in about five minutes the chat goes back to normal and keeps running.

**b8. :386-392.** "**In the window**, closing a chat asks Cancel, Close or Smart close; ...
offered to no shell tab ... Close is the default at one turn or fewer. The tab wears a breathing
amber mark while it wraps up; its menu's Cancel smart close, or the operator typing" →

> **In the window**, **End…** asks Cancel, End or Smart close, and closing a pane asks nothing
> and ends nothing; ... offered to no shell ... End is the default at one turn or fewer. The
> chat's pane head and row wear a breathing amber mark while it wraps up; **Cancel smart
> close** (the pane head's state button, and first in the chat's list), or the operator typing

**b9. :426-432, SI-8f (rewrite).** Today: "**Smart close puts the chat into the background**
(SI-8f): its tab shrinks to a fixed chip ... at the chat strip's left edge ... given up, ended or
refused, the tab comes back in its old place". →

> **Smart close leaves the chat where it is** (SI-8f, as amended by #4585): there is no strip
> chip, because a chat has no tab. Its pane head and row wear the breathing mark while it wraps
> up. Saved, the chat ends: its pane keeps the last screen with Resume and Clear pane and links
> the record, and a quiet notice offers **Open record**. Given up, ended or refused, the mark
> goes and the title bar's needs-you list says why.

**b10. :484, terminal editor (rewrite).** "opens in a shell tab of its own in the file's branch
folder, in front, run there as the same program and arguments" →

> opens as a split beside the focused pane, in the same tab and with the focus, in the file's
> branch folder, run there as the same program and arguments, never through a shell (#1042,
> #4588). When it exits the split goes and the focus comes back. It is never put back at a
> launch.

**b11. :504 and :561 (L2), budgets.** ❓ "Tab or pane switch ≤ 100 ms". The usual switch is now
a sidebar pick, which rebuilds a terminal. Its measurement is #4692. Recommendation: "Tab, pane
or sidebar pick ≤ 100 ms", with L2's "worst 48 ms" marked as measured before the pick existed.

**b12. :300.** "same planes, same tabs, same merges": project tabs. No change.

**b13. Strip `+`.** ❓ NOT RULED. D4: "A chat the person starts with `+` takes the focused
pane." Today the strip's `+` is named **New tab** (`menuWords.ts:40`, `StripMarks.tsx:72`,
`design-system.md:2254`). It isn't said whether that `+` stays and makes a tab (with an empty
pane?) or becomes New chat in the focused pane. Recommendation: the strip's `+` is **New tab**
and gives a tab with an empty pane, and the sidebar's `+` is New chat. That keeps D3's list
honest: a new tab only on a press that says so.

### docs/ui-primitives.md (27 items)

**b14. :284.** "the needs-you list and the tab's hand still show the wait" → "the needs-you list
and the chat row's hand still show the wait".

**b15. :286-300, a pane's rows (rewrite).** Today: "A pane's Notices stand in **a band of their
own at the top of the pane** ... **A pane is three rows** ... the band, on the terminal's own
fill with an 8px runway at its top so the chosen tab joins the terminal; the header (`PaneHead`)
... then the terminal". #4584 moved the band under the head.

> In a pane (`at="pane"`), a Notice is one box: its line, and under it what a way out opened.
> **A pane is four rows, top to bottom** (#4584): the task-question strip when a task asks
> (#4561); the header (`PaneHead`), with the chat at a glance at its start and the pane's
> actions at its end; the band of the pane's Notices, under the header so the header never moves
> when notices arrive; then the terminal, pinned to its bottom so a warning coming or going moves
> its top and never its last line. The header holds, left to right: the chat's icon, name and
> state mark, "harness · workspace", the state button, ⋯ (the chat's list), the bell's fixed
> slot, Split right, Split down and Close pane (`PaneActions`, which a view's pane draws too,
> #3147, #4585). The band takes about half the pane at most and scrolls inside past that. No
> Notice is ever drawn over what a chat wrote. *(Keep "They were in the pane's corner ..." as
> history, and add: "then at the top of the pane, until #4584 put it under the header.")*

**b16. :308-317, the cap of two (rewrite).** Today: "**At most two stand, and the rest are behind
"+N more"** ... with two at most, a third Notice and every one after it change only the count."

> **The band is minimised or expanded, and the choice is one app preference** for every pane,
> minimised by default and kept across restarts (#4584). Minimised, the bell in the header shows
> the count of the notes the pane holds, and the band shows only asks. Expanded, the band shows
> every notice, and its head row says what it holds and offers Dismiss all and Minimise ("7
> notices, 2 waiting on you · Dismiss all 5 · Minimise"). A consent or a blocking ask is always
> drawn as its full card, minimised or not, has no ✕, and is never counted. Dismiss all skips
> asks, and its label counts what it clears. The count is neutral, and takes the trouble colour
> when one note is trouble; expanded is the bell pressed (`aria-expanded`), never another
> number. A screen reader hears "5 notices, 1 in trouble, 2 asking". A new notice never opens
> the band or takes the focus, and its count is said politely. *(`MOST_ON_A_PANE` and the "N
> more wait in the Inbox" line go.)*

**b17. :1011.** "A preview the pointer's rest opened (a tab's task chip) takes no keyboard" → ❓
see b40; if the chip goes, the example becomes "(a sidebar row's hover card)" or the clause goes.

**b18. :1278, heading.** "The session tabs are drawn by the tabs family's parts" → "The strip's
tabs are drawn by the tabs family's parts".

**b19. :1280.** "The chat strip's tabs are drawn by `app/src/ui/tabs`" → "A workspace strip's
tabs are drawn by `app/src/ui/tabs`". (On "chat strip", see b33.)

**b20. :1299-1300.** "a press on the tab in front showing a task goes back to its chat, and a
drag's release is no press" → "and a drag's release is no press". A pane's earlier chat is now
⌘⇧H (#4699).

**b21. :1414, Menus rule 1.** "(`aria-label` on the content: "Tab console-developer 16")" →
"("Chat console-developer 16" on a chat's list, "Tab console-developer 16" on a tab's menu)".

**b22. :1443-1445, Menus rule 5.** ❓ "there is no greyed *Switch to* on the tab in front; on
any other tab it is the first row." *Switch to* has left the tab menu (#4585). The rule needs a
new example. Recommendation: "there is no *Go to <chat>* in the list of a chat already in the
focused pane". Also check against #4585 rule 2: "A row that applies but can't run now is shown
greyed with its reason". The two rules agree only if "can't apply" and "can't run now" stay
apart.

**b23. :1446-1448, rule 6.** "*Pinned*" as the toggle example → "*Pin tab*" (#4585's tab
menu). And "*End ⌫*": ⌫ on a focused tab now closes the tab (b30), so End's key needs checking.
❓ What End's key is now isn't ruled. Recommendation: none, and drop it from the example.

**b24. :1670.** "on the project tabs, the workspace tabs, the chat tabs, the panes" → "on the
project tabs, the workspace tabs, a workspace's tabs, the pane heads, the sidebar's chat rows".

**b25. :1692.** ""Ask ▸" on a chat's tab is the same shape" → ""Ask ▸" in a chat's list is the
same shape".

**b26. :1704-1706, a terminal's menu.** "then under a line the rows the panes' menu has there — a
new tab, the splits, ending the chat — so a right-click on a pane strands none of them." →

> then under a line the chat's list, then the pane's rows (Split right, Split down, Tile all,
> Send F2, Close pane), as a right-click on a pane head shows them (#4585), so a right-click in
> the terminal strands none of them.

#4585 lists Copy and Paste among the pane head's rows too. Here the terminal's own rows already
hold them, so they shouldn't come twice.

**b27. :1798.** "so a tab's `×`, a pane's `×` and the palette's rows all ask." →

> so End… asks from the pane head's list, the sidebar row's list and the palette alike. A tab's
> `×` and a pane's `×` end nothing and ask nothing (D2).

**b28. :1800-1806.** "**Three answers since Smart close (ADR 0064): Cancel, Close, Smart close,
in that order.** ... Close for one with at most one ... Shift+Tab from Cancel reaches Close." →
"Cancel, End, Smart close", "End for one with at most one", "reaches End". On a shell the dialog
has two answers, Cancel and End (#4585 rule 3).

**b29. :1840.** "a view is what a tab's pane holds when it does not hold a chat" → "a view is
one of the three things a pane holds: a chat, a view or nothing".

**b30. :1850-1851, an extension's view.** Add after "or its palette row": "Held ⌥ (Alt off a
Mac) on the button, or ⌥Enter on the palette row, opens it beside the pane it was pressed from
instead, never replacing what a pane shows (#4703, #4699 addendum)."

**b31. :2147-2150.** "**Each strip is ONE Tab stop** — projects, workspaces, chats ... arrowing
past a chat must not swap the panes under the operator." → "— projects, workspaces, a
workspace's tabs ... arrowing past a tab must not change what is on screen."

**b32. :2188-2195, Delete on a tab.** "**Delete on a focused project or chat tab presses the row
its `×` presses** ... ending a chat still asks first, a view tab still closes without asking" →

> **Delete on a focused project tab or tab presses the row its `×` presses** ... closing a tab
> ends no chat and asks nothing, a view tab still closes without asking

**b33. :2288-2292.** "A chat tab's `×` is outside the button, so a chat's name is centred" → "A
tab's `×` is outside the button, so its name is centred". "**Since 2026-10-12 the chat strip is
the editor's tab bar**" → "**Since 2026-10-12 a workspace's tab strip is the editor's tab bar**".
❓ "Chat strip" is used across the docs and in the `layer.chat` token. Recommend "tab strip" in
prose, with the token kept.

**b34. :2306.** "a chat's `×`, fresh mark and task chip" → "a tab's `×`" (see b40 for the chip;
the fresh mark moves to the row, a29).

**b35. :2558, Focus mode (rewrite).** "Focus mode (⌘⇧↩, `keys/focusMode.ts`) is one of its
keys." →

> **Focus this pane** (⌘⇧↩, Ctrl+Shift+Enter off a Mac, `keys/focusMode.ts`) is one of its
> keys (#4699). In a split tab it zooms the focused pane to fill the tab and hides the sides; in
> a one-pane tab it is the old Focus mode. The same key brings everything back, and the strip's
> tab wears a zoom mark meanwhile (#4583).

**b36. :2628-2632, the new-shell key.** "it presses the catalogue's own `shell.new` row" → "it
presses the catalogue's own `shell.new` row, whose shell takes the focused pane (#4588)". The
"new tab in GNOME Terminal" reason for the chord stays.

**b37. :2746-2749, Use skill….** "it opens Use skill… on the chat in front (in a split tab, the
pane with the keyboard) ... on each chat tab's menu row (`chat.useSkill`,
`tab.useskill:<tab>`)" → "on the focused pane's chat ... on that chat's list (`chat.useSkill`)".
The `tab.useskill:<tab>` id goes with the tab-keyed verbs (`tab-openers.md`, the palette table).

**b38. :2760.** "`⌘⇧J` is the tab's chats menu" → "`⌘⇧J` is the focused chat's tasks".

**b39. :2851-2898, the task keys (rewrite of the section's table and its next four paragraphs).**

> **The keys for panes, tabs and a chat's tasks take nothing either** (#1487, #4699,
> `taskKeys.ts`).
>
> |                                          | On a Mac        | Everywhere else                                                |
> | ---------------------------------------- | --------------- | -------------------------------------------------------------- |
> | The focused chat's tasks                 | `⌘⇧J`           | `Ctrl+Shift+J`                                                 |
> | Back to the chat this pane showed before | `⌘⇧H`           | `Ctrl+Shift+H`                                                 |
> | The pane in that direction               | `⌘⌥←` `↑` `→` `↓` | `Alt+←` `↑` `→` `↓`, left to the chat while a terminal has the keyboard |
> | The next / previous tab on the strip     | `⌘⇧]` / `⌘⇧[`   | `Ctrl+Shift+]` / `Ctrl+Shift+[`                                |
> | Focus this pane                          | `⌘⇧↩`           | `Ctrl+Shift+Enter`                                             |
> | Pick up the pane                         | `⌘⇧M`           | `Ctrl+Shift+M`                                                 |
>
> **⌘⇧J** is a menu of the tasks the focused pane's chat dispatched, and a pick shows the task in
> that pane. **⌘⇧H** swaps the pane back and forth with the one chat it showed before, like
> `cd -`. It does nothing when there is none, and when that chat is on screen in another pane it
> goes to that pane instead. Next and previous chat have no key: the sidebar and the palette do
> that job. Tabs wrap at the ends, as the strip's arrows do. Panes don't wrap: at the edge,
> nothing moves.
>
> Read in xterm.js 6.0.0 (`common/input/Keyboard.ts`): a `⌘` chord with a letter sends nothing
> but `⌘A`, and each arrow's case breaks on `metaKey` before it builds anything, so `⌘⌥` with an
> arrow sends nothing, while `⌥↓` without `⌘` is `ESC [1;3B` and stays the terminal's. `Ctrl`
> with a letter, `[` or `]` is encoded only when Shift is not held, so none of the `Ctrl+Shift`
> chords is a byte, and plain `Ctrl+J`, `Ctrl+H`, `Ctrl+[` and `Ctrl+]` still reach the shell.
> Off a Mac, `Alt` with an arrow is a byte (word movement), so it is the chat's while a terminal
> has the keyboard; Ctrl+Tab leaves the terminal first. `Ctrl+Alt` with an arrow is the
> desktop's. `⌘H` alone stays the system's Hide.
>
> When the keyboard follows a change into a pane, the terminal's own name says it ("steward 4,
> working, terminal"). When it doesn't (a drop into another pane, a jump to a chat already on
> screen, ⌘⇧H's jump), one polite line says "steward 4 is now in the right pane." An agent never
> moves a pane, so nothing an agent does is said this way.

The paragraph "**Where a task is drawn has keys too**" (:2867-2877): "`⌘`/`Ctrl`+Enter gives it a
tab of its own" → "opens it in a new tab". "On a task's own tab, the key that closes a tab ...
sends it back into its session's tab" goes. ❓ "Space opens it beside the session" and
"`Alt`+Enter opens it beside its session" aren't ruled for chats. #4699's addendum rules ⌥ for a
*view's* Open beside. Recommendation: ⌥Enter opens a chat beside the focused pane too, for one
convention; Space on a row is a plain pick.

The paragraph "**Down and Up on a Mac, because a tab's chats hang under it**" (:2879-2883) goes.
Replace it with one sentence: "`⌘⇧]` and `⌘⇧[` were kept free for the strip, and now are its."

:2893-2894 "(`tasks.menu`, `tasks.next`, `tasks.previous`, `tasks.own`)" → the new rows' ids.
❓ The ids aren't chosen.

**b40. :2899-2900, the tab's task chip.** ❓ NOT RULED. "**A menu that opens under a resting
pointer takes no keyboard** (#1487, `TabChip.tsx`). The task menu on a tab's chip". #4582's tab
shows the focused pane's icon and name, plus `⊟+N` for a split, and has no task chip. ⌘⇧J's menu
replaces the chip's menu. Whether the chip goes isn't said. Recommendation: it goes, and the
rest-opened rule moves to whatever hover card stays (the `⊟+N` hover lists panes). The same
question touches `design-system.md:1222` (`chip-explained`) and `:1725` (`.tab-tasks`).

### docs/ui-copy.md (2 items)

**b41. :27.** "*It needs sudo, so purlis types it in a shell tab and leaves running it to you.*"
→ "*It needs sudo, so purlis types it in a shell beside your chat and leaves running it to you.*"
❓ A one-job pane or a shell? See a18.

**b42. :58.** "the palette's *Rename chat console-developer 16…* is the tab's menu's *Rename…*"
→ "is the chat's list's *Rename…*".

### docs/design-system.md (7 items)

**b43. :1045 and :1087, `chats.scope`.** "`tab` (This tab) or `all`" → "`all`. An older value
`tab` reads as unset, so the Chats view opens on the workspace's (D5)". (`tab-openers.md` warned
`isScope` must fall back. This sentence is that fallback.)

**b44. :1045 (the example) and :1168-1170, `chats.tabbed`.** Delete `"tabbed": true` from the
example, and the sentence "And what pressing a task does (#1489, V100-74): **`tabbed`** ...". The
defaults list goes from four to three: "`false`, `true` and `false`". D8 refuses a setting for
the old behaviour.

**b45. :1171-1173, `chats.beside`.** "opens beside the chat, in its tab" → "opens in a split
beside the chat's pane, when a pane shows it".

**b46. :1222, `chip-explained`.** "the Notice the first dispatch shows to explain a tab's chip
of tasks (#1501)" → ❓ b40. If the chip goes, the cause retires and the key is read and ignored.

**b47. :1723-1725.** "`.tab-tasks`": same ❓ as b40.

**b48. :2254-2255.** "`New tab` with a `+` beside it is still `New tab`" stays true only if the
strip's `+` stays New tab (b13).

**b49. :2326-2328.** "What costs nothing is a tab opened beside the chat (the first task, the
repo's agent instructions, the harness setup), a `status` line, and a sign-in run in a shell tab
the operator can leave." →

> What costs nothing is a view opened in a tab of its own behind the chat (the first task, the
> repo's agent instructions, the harness setup), a `status` line, and a sign-in run in a split
> beside the focused pane, which goes when the sign-in exits.

### docs/plane-format.md (17 items; not named in the brief, but it's the record format)

**b50. reopen.json, :6225-6268 (new rows, and four rows retired).** #4587 rule 1 and its record
shape, #4700, #4701. Proposed rows (wording is the doc's voice; field names from #4587):

> | `tabs[]` | list | default `[]`; absent in a record written before it | one entry per tab, **in strip order**, each strip's tabs together. A record with no `tabs` is read the old way once (below) |
> | `tabs[].workspace` | str | default `""` | the strip the tab is on: a workspace's name, or empty for the strip of chats outside every workspace. A fact the person made, which nothing else knows (ADR 0036 as amended). A tab whose workspace is gone is dropped, and its chats stay in the sidebar |
> | `tabs[].pinned` | bool | default `false` | whether the operator pinned the tab (ADR 0039 as amended, #4700): its place first on its strip, and the tab across a relaunch and a cleared pane, but not its contents |
> | `tabs[].front` | bool | default `false` | whether it was its strip's front tab |
> | `tabs[].window` | int | default `0` | `0` for the project window, else the number of the tab window it was in (#4701); each tab window has one front tab of its own |
> | `tabs[].focus` | list[int] | default `[]` | the focused pane, as a path from the root: `0` the first child, `1` the second |
> | `tabs[].tree` | object | required | a **pane** or a **split**. A split is `{"split": "row" \| "column", "share": <0..1000>, "children": [<node>, <node>]}`, `share` being the first child's share in tenths of a percent, and exactly two children. A pane is `{"chat": <number>}`, `{"view": {…}}` (today's `views[]` fields without `workspace`, `at`, `active` and `pinned`) or `{"empty": true}` |
>
> The core refuses a record that names a chat twice across all of a project's tabs and windows,
> or a chat that isn't open, and reads such a tab as one empty pane. One-job splits are never
> written. Pane numbers are never written: a pane is named by its path.
>
> **Upgrade from a record with no `tabs`, read once:** chats that shared a `beside` become one
> tab in a flat row with even shares, in strip order. A pinned chat or view becomes a pinned
> tab. The tab that was in front at quit comes back as it was. Every other plain one-chat tab
> is dropped, and its chat lives in the sidebar. A workspace (or the plane root) left with no tab
> gets one, showing its first chat in the old strip order, pinned first, or an empty pane if it
> has no chats (#4587 rule 2). A view that was beside a chat was never recorded, so it's lost.

Rows that retire (kept only for the one upgrade read): `chats[].active` (replaced by
`tabs[].front`), `chats[].tab_opened`, `chats[].shows`, `chats[].beside`, and `views[]` with its
nine rows (now `{"view": …}` panes). ⚠ Gap found: `chats[].pinned` exists in code
(`reopen.rs:93-109`, "Whether the operator pinned this chat") and isn't documented here. It
retires to `tabs[].pinned` (#4700).

`chats[]`'s order: "in the order the chat strip drew them, which is the order a launch puts them
back in (... the operator can drag a tab)" → "in the sidebar's order, which a launch keeps; the
strip's order is `tabs[]`'s".

`chats[].label` (:6235): "which its tab says instead of" → "which its row and pane head say
instead of". `chats[].graded` (:6238): "It has no tab and is listed nowhere" → "It has no pane
and is listed nowhere".

**b51. :6204-6206.** "how many chats and view tabs" → "how many chats and tabs"; "holding no
chat and no view tab" → "holding no chat and no tab".

**b52. :610.** "the Notice on the asking chat's tab" → "the asking chat's ask (its row, the
Inbox, and its pane when one shows it)".

**b53. :2192.** "**Unlink work item** on a chat tab's menu and in the palette" → "in a chat's
list and in the palette".

**b54. :2294.** "chat-name: <the name its tab shows | unknown>" → "<the name its row shows |
unknown>".

**b55. :2380.** "Deleting one costs at most a tab that does not close by itself." → "Deleting
one costs at most a smart close that does not end its chat by itself."

**b56. :2401.** "The app closes a tab on the line only while that chat is being smart-closed" →
"The app ends a chat on the line only while it is being smart-closed".

**b57. :3522.** "mark on its chats' tabs" → "mark on its chats' rows and pane heads".

**b58. :5520.** "the window marks a chat's tab when `CLAUDE.md`, ..." → "the window marks a
chat's row and pane head when ...".

**b59. :5892.** "Allow for everyone on a chat's tab" → "Allow for everyone on a chat's grant
ask".

**b60. :6046, :6295, :6352, :6411.** "dispatch from a chat's tab", "dispatched from that chat's
tab themselves", "started from a tab" (twice) → "dispatch with Ask <persona>… on a chat",
"dispatched from that chat themselves", "started themselves" (twice).

**b61. :6847.** "for a tab's tasks' tokens (#1500)" → "for a chat's tasks' tokens (#1500)".

**b62. :7880, `layout.json`'s `chats`.** Drop "and #1489: `tabbed`, whether a pressed task opens
in a tab of its own, where otherwise its session's tab is switched to it"; and "`beside`,
whether a browser a chat starts opens beside the chat" stays.

**b63. :7903.** "the shims that warn when a harness is started by hand in a shell tab" → "in a
shell" (a25).

**b64. :8223.** "number as its tab says it" → "number as its row says it".

Left alone: :1792 and :8289 (the frozen Python charter's frame), and the rest (Settings tab,
memory tab, Dispatches tab, Activity tab, Grades tab, Review tab: views).

### docs/getting-started (public on the site: `site/src/pages.mjs` builds every `docs/**/*.md`) (5 items)

**b65. github.md:12, gitlab.md:16.** "**Install** runs it in a shell tab." → "**Install** runs it
beside your chat, and it goes when the installer is done." ❓ a18.

**b66. github.md:91.** "**Sign in to GitHub** opens a shell tab running `gh auth login`." →
"**Sign in to GitHub** runs `gh auth login` beside the page, and it goes when you're signed in."
❓ a18: a button-run sign-in isn't named by the ruling, and from the welcome there's no pane.

**b67. gitlab.md:84-85.** Same as b66 for `glab auth login`, and "add `--hostname <host>` in that
tab" → "in that terminal".

**b68. github.md:160, gitlab.md:163.** "**Chat:** one conversation with an agent, in a tab." →

> **Chat:** one conversation with an agent. It's listed in the sidebar, and you show it in a
> pane by picking it.

github.md:65 and gitlab.md:67 ("its tab": the project's) stay.

### docs/updating.md, docs/uninstall.md (2 items)

**b69. updating.md:68.** "writes down every chat and view tab each open project holds" →
"writes down every chat and tab each open project holds". (:315 is a vault's tab: stays.)

**b70. uninstall.md:147.** "the shims a shell tab finds first on its `PATH`" → "the shims a
shell finds first on its `PATH`". (:42 is a vault's tab: stays.)

### Other crate docs (8 items)

**b71. personas.md:146.** "ask the operator to allow the vault on the chat's tab" → "ask the
operator to allow the vault, which they do where the chat's ask shows (its pane, its row, the
Inbox)".

**b72. personas.md:205-210 (rewrite).** "It is told that only the person answers it, in the
task's own tab ... The window says the same on the session's tab: its chip wears the hand from
the moment the prompt is held, and its pane names the task, with **Show the task**" →

> It is told that only the person answers it, in the Inbox or in the task's pane, so that it
> says so where the person is; nothing it does answers the prompt. The window says the same on
> the session: its row wears the hand from the moment the prompt is held, and its pane, when one
> shows it, names the task with **Show the task**, until the task gets past the prompt, its turn
> ends or it ends.

**b73. personas.md:356.** "the chat's tab shows a Notice naming the server" → "the chat shows a
Notice naming the server, in its pane's band and in the Inbox". ❓ NOT RULED: #4584 and #4588
rule where a paneless chat's *asks* go, but not its plain notices (this one, a Retry). The
recommendation is the Inbox's Notices.

**b74. secrets.md:59-63.** "the chat's tab shows the same sandbox block Notice ... it can be
allowed on the tab" → "the chat raises the same sandbox block ask, on its row, in the Inbox and
in its pane when one shows it ... it can be allowed in the window".

**b75. secrets.md:111.** "the chat's tab shows a notice with **Allow {persona} to use this
vault**" → "the chat raises an ask with **Allow {persona} to use this vault**". secrets.md:349
"however many chats and tabs ask" → "however many chats and views ask". The other 23 hits are a
vault's tab: they stay.

**b76. hooks.md:948.** "is marked on its tab in the window instead" → "is marked on its row and
pane head in the window instead".

**b77. control-plane.md:561.** "opens a shell tab that runs `claude auth login`" → "runs `claude
auth login` beside the page, and it goes when the sign-in is done". ❓ a18.

**b78. control-plane.md:588-592.** "the chat's tab saying so" (twice) → "the chat's pane head
saying so". ❓ placement as a29. (:577 is the persona's view tab: stays.)

**b79. themes.md:606-612, Session tabs.** ❓ NOT RULED, theme format. The heading becomes
"Strip tabs" (prose only). The `tabs` family's variants are `chat`, `view`, `file` and
`background`. `background` is Smart close's strip chip, which #4585 removes. An empty pane's tab
("New chat" with a plus, #4582) may want a variant of its own. Changing a theme's variants is a
change to a versioned theme manifest, which the rulings don't decide. Recommendation: keep
`background` as accepted and unused for one manifest version, and add `empty`.

---

## (c) crates/purlis-core/docs/handoff.md

### c1. The section on how a handoff is seen, :91-123 (rewrite in full)

Step 5 (:96-98) today: "The chat lands as a new tab on the target workspace's strip, **behind the
tab you are reading**, and the window is not raised. It takes the front only in a window with no
tab at all, where there is nothing to interrupt."

> 5. The chat starts in the target workspace's Chats list, **in no pane**, and the window is not
>    raised: nothing you are reading is replaced, and the focus stays where it was. Its arrival
>    is said in three places, in this workspace or another:
>    - a card on the target workspace's chip in the title bar: "<chat> arrived from <asking
>      chat>", with **Go to it**. It takes no focus. A press anywhere else hides it and leaves a
>      dot on the chip, and pressing the dot brings it back. A workspace with no chip drawn has
>      the card in the Inbox's Notices instead, with a way to open the workspace;
>    - an **Update** in the Inbox, which isn't counted on the ✋, because a handoff asks nothing;
>    - a **new** mark on the chat's row, until you have looked at the chat.

The paragraph at :108-115 today ends: "There is no mark on the strip beyond the new tab itself:
the tab is how you see it."

> The todo comes after the chat is open, and a failure to write it is said and never undoes
> the open. A handoff the app would not open writes none. **A handoff that was held for a grant
> and then allowed has its todo written by the app** (#1471): the command returned when the
> handoff was held, and what could not be written is said to the asking chat with the start.
> **The chip's card, the Update and the row's mark are how you see it.** None of them takes a
> pane, and none can be turned off.

The paragraph at :117-123 today: "That is why a handed-off chat always lands as a tab you can
see, stamped with the chat it came from".

> **The app opens one chat per `purlis handoff`**: *(unchanged to "with `claude -p`.")* That is
> why a handed-off chat is always announced where you can see it, stamped with the chat it came
> from, and why what consents to it is the app's own decision and never a word the chat sends.

This keeps the safety property the tab gave (#4588 rule 6): a chat started by another chat
always reaches where the person can see it. ❓ "none can be turned off" is a recommendation, not
a ruling. It's what keeps the property from being undone by a setting.

### c2. Word swaps and one-sentence changes (39 items)

- **:35** "a chat that works on its own, with a tab of its own, and this chat hears nothing
  back" → "a chat that works on its own, on a row of its own in the Chats list, and this chat
  hears nothing back".
- **:58** "`--name` is what the new chat is called — its tab, and wherever else" → "— its row,
  its pane head, and wherever else".
- **:162** "The same note is on the new chat's tab, as its tooltip, and in its pane's corner" →
  "The same note is on the new chat's row, as its tooltip, and on its pane head when a pane
  shows it".
- **:179** "the window shows it as a task of this chat, not as a tab that moved away" → "…, not
  as a handoff".
- **:242, :686, :840, :899, :958** "a task you started from a tab", "from a chat's tab", "a task
  you started from a tab", "with Ask from a tab", "asked for from a tab" → "a task you started
  yourself", "with Ask <persona>… on a chat", "a task you started yourself", "with Ask
  <persona>…", "asked for themselves".
- **:269-270** "**Closing a task's tab ends nothing**: the tab goes, and the task goes back to
  the Chats list and keeps working." → "**Closing a task's pane ends nothing**: the pane clears,
  and the task keeps working in the Chats list."
- **:275-276** "It is on the session's row menu and its tab menu, beside Stop with everything
  below it, and on the last line of the tab's task menu, under "End a task"." → "It is in the
  session's list (its pane head's ⋯, its row's ⋯ and the palette), beside Stop with everything
  below it, and on the last line of its tasks menu (⌘⇧J), under "End a task"."
- **:299** "You can still ask a persona from its tab yourself." → "You can still ask a persona
  yourself, with Ask <persona>… on it."
- **:331** "A Notice on the asking chat's tab says who wants to dispatch to whom" → "An ask on
  the asking chat says who wants to dispatch to whom: on its row, in the Inbox, and in its
  pane's band when a pane shows it. It".
- **:351** "and has a tab once you open it" → "and is shown in a pane when you pick it".
- **:368** "with nothing to allow on its tab" → "with nothing to allow".
- **:393** (table) "until you allow them on its tab" → "until you allow them on it".
- **:443** "the person starts such a chat from the asking chat's tab" → "the person starts such a
  chat with Ask <persona>… on the asking chat".
- **:580** "and the asking chat's tab says it to you" → "and the asking chat says it to you, on
  its row and in its pane when one shows it". ❓ a plain notice on a paneless chat (b73).
- **:660, :665, :669** "Its tab's menu ends with how many of its tasks run", "the number its tab
  menu counts against", "said on its tab menu's footer too" → "Its tasks menu (⌘⇧J, and its
  row's) ends with …", "the number its tasks menu counts against", "said on its tasks menu's
  footer too".
- **:760** "A task you started yourself from this chat's tab (*Ask <persona>…*)" → "A task you
  started yourself on this chat (*Ask <persona>…*)".
- **:790** ❓ "A wait on a task whose tab you closed says how it ended." Under the old rules
  closing a task's tab ended nothing, so the sentence is already unclear. Recommendation: "A
  wait on a task you closed (Close now) says how it ended."
- **:830** "type in the task's own tab" → "type in the task's pane".
- **:850** "whatever the task's tab is showing" → "whatever the task's pane is showing".
- **:882** "a chat started again in that tab cannot report for the task" → "in that pane".
- **:890-891** "A task you have in front of you is not ended under you; it is ended when you move
  to another chat." → "A task on screen is not ended under you: one in a pane of the front tab
  of any of the project's windows. It is ended once no pane on screen shows it." (#4587 rule 4,
  #4701)
- **:908-909** "**Reopen makes a finished task an ordinary chat.** It resumes the task's
  conversation in a tab of its own." → "… It resumes the task's conversation in the focused
  pane." (D1: the person pressed it.)
- **:925** "If the chat whose tab you asked from has closed" → "If the chat you asked from has
  ended".
- **:928** "(a permission prompt, a question, in its own tab)" → "(a permission prompt, a
  question, on its row and in the Inbox)".
- **:980-982** "**Ask <persona>…** on a chat's tab menu and in the palette ... under the chat
  whose tab you used." → "**Ask <persona>…** in a chat's list (its pane head's ⋯, its row's ⋯)
  and in the palette ... under the chat you used it on."
- **:992-993, the stamp.** ⚠ format. "``⟨the person asks, from the tab of `steward 3` · …⟩``" →
  "``⟨the person asks, from `steward 3` · workspace alpha · 2026-10-07 14:32⟩``". The string is
  built in `handoff.rs:488` and pinned by tests (`:1212-1241`, per `tab-openers.md`), and every
  chat reads it. "No chat's stamp opens with those words" still holds. The doc and the code
  change together.
- **:1000** "the Notice on that chat's tab has *Dispatch to <persona>…*" → "the vault's ask on
  that chat has *Dispatch to <persona>…*".
- **:1005-1008** "the entry is not on any tab ... *Ask devops…* is not on the tab of a chat
  running as `steward` ... asking from that tab would send" → "the entry is in no chat's list
  ... *Ask devops…* is not in the list of a chat running as `steward` ... asking from that chat
  would send".
- **:1107** "The work moves, the new chat opens as a tab, and it owes no report." → "The work
  moves, the new chat starts in the Chats list and is announced (step 5, above), and it owes no
  report."
- **:1115** "raises a Notice on the asking chat's tab, with **that first brief in full**" →
  "raises an ask on the asking chat, with **that first brief in full**".
- **:1132** "every command of its that asks you still asks you, in its own tab. The brief is the
  first message in that tab" → "… still asks you, on its row and in the Inbox. The brief is the
  first message in that chat".
- **:1313** "nothing appears on the chat's tab" → "nothing appears on the chat".
- **:1344** "Every command of its that asks still asks, in its own tab" → "Every command of its
  that asks still asks, on its row and in the Inbox".

Left alone in handoff.md: the Dispatches tab (:343, :498, :586, :1063), a task's Changes tab
(:485, :540-575), the Activity tab (:821, :835) and "line breaks and tabs" (:832). That's 17
hits, all views or characters.

---

## (d) Site and READMEs

**d1. site/src/content/docs/index.mdx:25** (the public home page's first card). Today: "Claude
Code, Codex and opencode chats run side by side, each in its own tab." →

> Claude Code, Codex and opencode chats run side by side. Every chat is listed in the sidebar,
> and you split the window to watch several at once.

That's the only hit in `site/src`. But the site also publishes every `docs/**/*.md`
(`site/src/pages.mjs`, `markdownPages`). So `docs/getting-started/*.md` (b65-b68), `spec.md`,
`ui-primitives.md` and the rest are public too, and `CONTEXT.md` and `README.md` are on GitHub.
Rewording them is public. Per the standing rule it's not an operator call, since it only
describes shipped behaviour. It should land with the build, not before it, or the public docs
describe a window nobody has.

**d2. README.md:40.** "**Chat**: one conversation with a coding agent, in a tab, in one
workspace or at the project root. It is where work happens, and the window says which chats need
you." →

> - **Chat**: one conversation with a coding agent, in one workspace or at the project root.
>   Every chat is listed in the sidebar, and picking one shows it in a pane; closing the pane
>   leaves it running. It is where work happens, and the window says which chats need you.

AGENTS.md and CLAUDE.md (root): no hits. No AGENTS.md, CLAUDE.md or README.md in `app/`.
CONTRIBUTING.md:222 ("a tab moved") still holds.

---

## Gaps and contradictions, in one list

1. ⚠ **Level-1 chats in no pane** (a10). "Headless chat: never a level-1 chat" guarded a chat
   that can't report asks. D1 and D2 let any chat leave its pane. Only the faint hand covers it.
2. ⚠ **`spec.md:75` "free split panes"** contradicts D6's "not a free grid" (b3).
3. ❓ **The strip's `+`** (b13, b48): New tab or New chat.
4. ❓ **A tab's task chip** and its `chip-explained` Notice, `.tab-tasks`, the rest-opened menu
   (b17, b40, b46, b47).
5. ❓ **Where you answer** has no meaning once tasks have no tab (a7).
6. ❓ **Show-more's needs-you count** once the red count moves to the row (a20).
7. ❓ **A paneless chat's plain notices** (not asks): an MCP server's Retry, two tasks in one
   folder (b73, c2 :580). #4584 and #4588 rule only asks.
8. ❓ **Marks without a slot on #4585's pane head**: plane updated, sandbox level, a version
   change (a29, a30, b78).
9. ❓ **One-job programs started by a button**: Install, Sign in to GitHub/GitLab, a profile's
   Sign in, a sudo line, and any of them from the welcome, where there is no pane (a18, b41,
   b65-b67, b77).
10. ❓ **A browser's thumbnail** on a chat tab (a34).
11. ❓ **"Notices centre"** in #4588 rule 6 isn't a glossary word: the Inbox lists Notices (a5).
12. ❓ **Keys not ruled for chats**: Space and ⌥Enter "beside" on a task row; End's ⌫ in menu
    rule 6; the new catalogue ids (b23, b39).
13. ❓ **Menu rule 5's example** (*Switch to*) is gone and needs a replacement (b22).
14. ❓ **Theme manifest** `tabs` variants: `background` loses its use, and `empty` may be wanted
    (b79).
15. ❓ **Kill switch then Resume** on the ended band before re-arm (a24).
16. ⚠ **The person's stamp** "from the tab of `x`" is a format with tests, read by every chat
    (c2 :992).
17. **"Chat strip"** as a prose name, against the tab strip and the `layer.chat` token (b33).
18. **`chats[].pinned`** is in the code and not in `plane-format.md` (b50).
19. **L2's budget** names a tab or pane switch. The real switch is now a sidebar pick, measured
    under #4692 (b11).
