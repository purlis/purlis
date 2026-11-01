# What opens or assumes a tab today (#4580)

Research for the map #4575, against the decisions in #4576. Everything cited was read at
`7e723d5d4` (origin/main, 2026-11-02). Nothing was run: no test, no app. "Breaks" below is read
off the code, not observed.

Decisions are cited as D1 to D9, the numbered answers in #4576.

## The short answer

Three things in the code stop most chats living without a tab. The rest is rewording or a
changed call.

1. **The core says every chat but a task has a tab.** `Chat::has_tab` (`reopen.rs:280`) is true
   for all but an unopened task, and the launch draws a tab for every chat it's true for
   (`PlaneView.tsx:1646`). Until that changes, D4's handoffs and D8's upgrade can't hold: a
   relaunch gives every chat its tab back.
2. **A pane is owned by a chat.** `Content` is a session or a view, with no empty kind
   (`tabs.ts:251`), so D2's "closing a pane clears it" has nothing to become. `Tab.shows` says
   what a pane shows *in place of its own chat*, `homeOf` finds a chat through who asked whom
   (`tabs.ts:1279`), and the core only records a pane on its own chat's entry. D1 needs a pane
   that can show any chat. Today's model only lets a pane show a task below its own chat.
3. **Most chat actions are addressed by tab id.** Rename, pin, fresh, restart, move, work link,
   ask persona, use skill and end are verbs carrying `tab: number` (`actions.ts:338-366`), with
   their rows built by loops over `now.tabs.order` (`actions.ts:1980-3105`). A chat with no tab
   has none of these rows today. D9 makes that the normal case.

**One thing #4576 may not expect: splits aren't saved.** `reopen.rs` records no tree of panes. It
keeps, per chat, `shows` and `beside`, and per view tab a place on the strip. `tabs.ts:405` says
so: "A relaunch brings each chat back as a tab of its own". So D8's "tabs with a split stay" can
only be honoured at the upgrade for a task beside its session (`beside`). Two plain chats split
together, or a chat split beside a view, aren't in the record to keep. D6's layouts also don't
survive a relaunch until a layout record exists.

## The table

Verdicts: **Holds** works as it is. **Changes** the call changes, and nothing else leans on it.
**Breaks** can't work once most chats have no tab.

### Chats that start without the person

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| Dispatched task | No tab: `arrived.from?.tab === false` returns early (`PlaneView.tsx:1850`), and `has_tab` is false for `Mode::Task` (`reopen.rs:280`, `:409-415`). Listed under the asker. | Same (D4). | **Holds.** The working precedent: a chat already runs, asks and reports with no pane. |
| Handoff | `openTabBehind` on the target workspace's strip, not in front (`PlaneView.tsx:1833-1863`, `tabs.ts:881`). `Mode::Handoff`: "the chat opens as a tab" (`reopen.rs:410`). | Sidebar only, never a pane (D4). | **Breaks.** See below: nothing else tells the person it arrived. |
| Chat restored under a new number | `openTabBehind` unless `tab === false` (`PlaneView.tsx:6384-6396`). | As its old self: tab only if it had one. | **Changes**, follows `has_tab`. |
| Grade's chat | Never drawn or listed (`reopen.rs:174`, `graded`). | Same. | **Holds.** |

### Chats the person starts

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| `+` / New chat | `ask({tab:true})` (`PlaneView.tsx:3087`) to `openTab` in front (`:3503`). | Takes the focused pane (D4). | **Breaks** with an empty window: after the upgrade the strip can be empty, and with no tab there's no focused pane (`focusPane` and `frontTab` need a tab, `tabs.ts:1101`). See "not decided". |
| Split a pane | `splitFocusedPane` (`PlaneView.tsx:3509`). Starts a new chat beside. | Stays in its tab (D3). | **Holds.** |
| New shell | `openTab` (`PlaneView.tsx:3117`). | **Not decided.** D3 doesn't list shells, D4 says only "a chat the person starts with `+`". | **Changes**, needs a ruling. A shell is listed in the Chats section (`chatsTree.ts:147`), so it can live there. |
| Terminal editor (#1042) | `openTab` from the core's event (`PlaneView.tsx:3218`); `spec.md:484` "a shell tab of its own". | **Not decided**, as above. It's opened to edit a file, so replacing the focused pane is surprising. | **Changes**, needs a ruling. |
| Curation action, theme chat, skill chat, "Start a chat here", first task, a Fix's chat | `openTab` in front (`PlaneView.tsx:5440`, `:5485`, `:5575`, `:5621`, `:5659`, `:4188`). | Not named in #4576. The person pressed for each, so D1's rule fits: the focused pane. | **Changes**, six calls to one new function. |
| Resume, reopen finished task, retry a chat that wouldn't start | `openTab` in front (`PlaneView.tsx:5710`, `:5784`, `:6347`). | Focused pane. | **Changes.** Docs say "a tab of its own" (`handoff.md:897`). |
| Start fresh, restart | `replaceSession` keeps the pane (`tabs.ts:1211`). Falls back to `shows` for a task. | Same. | **Changes**, reachable only with a tab today (see Palette). |
| Ask a persona | Verb carries a tab (`actions.ts:357`). The task it starts stays tabless. | Lives on the chat (D9). | **Breaks** with the verbs. |

### Ways of going to a chat

All of these end in `showChat` (`PlaneView.tsx:3803`).

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| `showChat` itself | Finds `homeOf` by lineage, and `switchTabTo`s it. **With no home it calls `ownTab`, which adds a tab and tells the core to keep it** (`:3807-3813`, `:3714-3733`). | On screen: jump. Otherwise replace the focused pane (D1). Never add a tab. | **Breaks.** Applied to most chats, this rebuilds the strip D3 is removing. |
| Sidebar row | `onOpen={showChat}` (`PlaneView.tsx:9499`). | D1. | **Breaks** with `showChat`. |
| "Tasks open in their own tabs" setting | `chatsListPrefs().tabbed` (`:3809`, `settings/you.tsx:230`). | Not listed by D3, and D8 refuses settings for old behaviour. | **Breaks**: remove it. Its stored value goes too. |
| Notices' "Go to it" | `Notice.tsx:344`, `onGo` to `onShowChat` (`LayoutPanes.tsx:389`). | D1. | **Works through `showChat`**, but see the next row. |
| Which notices a pane draws | `frontOthers` (`PlaneView.tsx:7319`) and `chatsOfPane` (`tabChats.ts:52`): the pane's own chat and tasks at home in it, by lineage. | D7 says a blocking ask stays open on the pane. | **Breaks.** A sidebar-only handoff has no pane to ask on, and a chat shown in someone else's pane has no lineage to it. See "not decided". |
| Needs-you: Inbox Go | `press(queueRow)` or `showChat` (`PlaneView.tsx:9644-9649`), and `showNeeding` (`:3882`). | D1. | **Holds** once `showChat` does. Its fallback text "That chat has no tab in this window" (`actions.ts:3810`) rewords. |
| Needs-you: another project's | `InboxElsewhere.tsx:180`, `App.tsx:2169-2203`: raises the project and runs `showChat`. | D1. | **Holds** on `showChat`. |
| Failed task's Go | `goToEnded` (`:3836`). Finds a row, or falls to the asker. | Same. | **Changes** a little: "a task with no asker in a tab" stops being special. |
| Red count on a tab | Items are "counted in red on its tab and on any show-more hiding it" (`CONTEXT.md:1091`). | A tabless chat has no tab to count on. Its row and ✋ still count. | **Changes.** |
| OS notification click | Opens the Inbox at the chat's group, no tab (`asknotify.rs:47-53`, `:627`). `chat_on_screen` via `looks_at`. | Same. | **Holds.** `looks_at` must learn panes that aren't lineage (`chats.rs:4727`). |
| Away summary, Activity, handed-off rows | `showChat` (`AwaySummary.tsx:91`, `ActivityTab.tsx:313`, `actions.ts:4148`). | D1. | **Holds** on `showChat`. |
| Next/previous chat in this tab | Keys and rows walk one session's tasks (`taskKeys.ts`, `actions.ts:4708`). `inside` keeps them in the tab (`:3807`). | Open on the map ("Keyboard"). | **Breaks** as words and meaning: "this tab's tasks" has no referent once a pane shows anything. |

### The palette and menus

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| Switch to a chat | `Switch to tab X` per tab (`actions.ts:1980-1987`). Rows for tasks (`taskRows`, `:4117`), handed-off chats and needs-you. **No Show row for a chat that's neither.** | Find any chat (#4575 keeps finding in the palette; ADR 0036 says the palette lists every chat). | **Breaks.** A plain chat's only palette row today is its tab. |
| Chat actions | `tab.rename`, `tab.pin`, `tab.fresh`, `tab.restart`, `tab.move`, `tab.worklink`, `tab.useskill`, `tab.activity`, `tab.network`, `tab.close`: all per tab (`actions.ts:1994-3105`). Listed chats have only stop, end, place, brief, answer and restart (`:3110-3118`). | Same list on header, row and palette (D9). | **Breaks.** Largest single change: `Does` and `Doing` move from tab to session. `restartListed` is the precedent (`:4171`). |
| Close | `tab.close` with `ends: true`, note `ENDS_IT` (`actions.ts:3105`). `close` ends every session in it (`PlaneView.tsx:3664-3680`). `closePane` ends too (`:3638`). The danger cue reads `ends` (`menuModel.ts:169`, `StripMarks.tsx:46`). | Close clears; ending is separate; cue goes (D2). | **Breaks.** The core refuses too: `close_tab` says "closing its tab ends it" (`chats.rs:4791`). |
| Smart close | `beginSmartClose` sends the tab to the background (`PlaneView.tsx:8183-8193`), a chip on the strip (`tabs.ts:949`, `:1011`). `tabs.closeChat` runs when the record lands. | Chat action (D9); it no longer closes a tab. | **Changes.** With no tab it already skips the chip (`goes` is false). The chip's future is a prototype question; ADR 0064's title says "closes the tab". |
| Pin | `pinTab`; stored on the chat (`Chat::pinned`), read as the tab's (`tabs.ts:977`). | D3: a pin keeps a tab, doesn't make one. | **Changes**, and it's on the map as open: chat pin and tab pin are one bit today. |
| "This tab" filter | `chatsScope.ts:122`, `ChatsSection.tsx:204`, `:391`. | Goes (D5). | **Changes.** A stored `"tab"` scope must fall back, or `isScope` rejects it and it reads as unset (`chatsScope.ts:131`, `:160`). Check that's the wanted default. |
| App menu | Preferences opens the Settings view (`lifecycle.rs:27`). | Same. | **Holds.** |

### Views

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| Extensions' `open_view` | **Not a tab opener.** It only answers what a view shows (`Views.tsx:173`, `:271`). An extension can't open anything: the person presses a button (`Views.tsx:548`) or a palette row (`actions.ts:2936`, `:2957`), which is `openView`. | A view gets a tab (D3). | **Holds.** The map's question (pane or tab for an extension) is about a capability that doesn't exist today. |
| `openView`, `offerView`, `openPreview` | Tab in front, tab behind, preview tab (`tabs.ts:479`, `:509`, `:525`). 23 palette rows, Settings, personas, vaults, memory. | D3. | **Holds.** `offerView` opens tabs nobody pressed for (first task, harness setup, `PlaneView.tsx:4311-4324`); D3 allows it as a view. |
| Views beside a chat | `openBrowserBeside` via `openViewBeside` (`tabs.ts:1696`, `browsers.ts:74`). Looks for the chat in `content.session` only. With a tabless chat it does nothing. | D1: a chat shown in a pane should qualify. | **Breaks** quietly: the setting does nothing for a chat shown by `shows`. Already true of tasks today. |

### Restore and upgrade

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| Launch | A tab per `tab !== false` chat, then views by `at`, `beside`, `shows` (`PlaneView.tsx:1643-1700`). | Tabs from a saved layout; everything else in the sidebar (D8). | **Breaks.** Needs a new `has_tab` or none, and a layout record. |
| The record | `tab_opened` (`reopen.rs:168`) means "a task the person opened". `false` is every old record, so reusing it as "has a tab" closes every chat, pinned included. | D8. | **Breaks**: needs a real migration, not the "absent is false" habit. |
| Front chat | `in_front` is one chat (`chats.rs:137`); `front` picks the tab (`PlaneView.tsx:1718`). | After D8 that chat may have no tab. | **Breaks** at the first upgrade launch: nothing to bring forward. |
| Strip order | `chat_order` by session (`lib.rs:2689`). | Tab order, since tabs aren't chats. | **Changes.** |
| `tab_shows` | Keyed by the pane's own chat (`lib.rs:2665`, `chats.rs:4817`). Restore checks the shown chat is below it (`PlaneView.tsx:1691`). `dispatched.rs:939` and `dispatchrestart.rs:267` repair it. | A pane has no own chat. | **Breaks.** |
| Chat ended on screen | `closeChat` takes the pane away (`tabs.ts:1165`, `PlaneView.tsx:2178`, `:2211`, `:2439`). | D2's cleared pane looks right, but #4576 doesn't say. | **Changes.** |

### CLI and deep links

| Path | What it does today | Under #4576 | Verdict |
|---|---|---|---|
| CLI | **No command opens or focuses a chat.** `purlis handoff` and `dispatch` ask the app for a chat; the words are "opened chat N" (`purlis-cli/handoff.rs:305`). `session.rs:262` tells the operator a tab will close. | Reword only. | **Holds.** |
| Deep links | **None.** No `deep-link` plugin in `Cargo.toml`, `tauri.conf.json` or `package.json`. Second launch hands over a folder (`lib.rs:532`, `:3544`). "Deep links into Settings" are in-window (`settings/links.ts:16`, `:227`) and open a view. | D3. | **Holds.** |
| UI RPC | `open_chat_tab`, `close_chat_tab` (`session-protocol/ui.rs:1067`, `:1115`). | Follow the core. | **Changes.** |

### What agents and docs are told

Wording, but it's the agent-facing kind: these are read as instructions.

| Where | What it says | Verdict |
|---|---|---|
| `plugin/skills/handoff/SKILL.md:27`, `:70`, `:113-115`, `:133` | "gets a tab of its own", name it for "somebody scanning a strip of tabs", "the new chat is a tab in that workspace" | **Wrong under D4.** The naming advice survives if it's about the Chats list. |
| `plugin/skills/persona/SKILL.md:57`, `secrets/SKILL.md:39`, `:55`, `:121-134`, `safe-remove/SKILL.md:74-77` | "a notice on this chat's tab", "this chat's tab" | **Wrong for a chat in no pane.** Vault tabs are views and stay right. |
| `plugin/skills/smart-close/SKILL.md:10`, `:78-80`, and `session.rs:262`, `:352` | "closes this tab", "the tab is theirs to close" | **Wrong under D2.** |
| Chat stamp `handoff.rs:488` | "the person asks, from the tab of `x`" | **Wrong** when asked from a sidebar row. A format every chat reads, with tests on the string (`:1212-1241`). |
| Refusal strings: `dispatchtalk.rs:1169`, `:1220`, `dispatched.rs:385`, `:707`, `:714`, `awareness.rs:948`, `dispatchdecision.rs:266` | "in its own tab", "type in the tab of '{name}'" | **Wrong.** Sent to agents, who relay them to the person. |
| `crates/purlis-core/docs/handoff.md` | 64 lines mention a tab. Key: `:35`, `:96`, `:115` ("the tab is how you see it"), `:351`, `:1096`, `:1121` | **Rewrite** the handoff section; the rest is word swaps. |
| `CONTEXT.md` | 63 lines. Key: `:35` (Chat: "in a tab"), `:849-856` (Task), `:898-899` (Handoff), `:985` (Headless chat), `:1091` | **Rewrite** Chat, Task, Handoff, Headless chat. "Headless chat" is nearly the new normal. |
| `docs/spec.md` (35 lines), `docs/getting-started/{github,gitlab}.md:160`, `personas.md:206`, ADRs 0036, 0039, 0043, 0064 | "one conversation with an agent, in a tab" | **Amend**: on the map as open. |

## Why the handoff row matters

A handoff is the one case where the tab *is* the announcement. `handoff.md:115`: "There is no
mark on the strip beyond the new tab itself: the tab is how you see it." The Handoff row in the
Chats list says `from <chat>`, but it's on the target workspace, and a person on the Workspace
filter won't see it arrive in another workspace. D5 keeps Workspace and All, and D4 removes the
tab, so a cross-workspace handoff now arrives silently. Tasks don't have this problem: they're
under the chat that asked, which the person is reading.

The same goes for a handoff's first ask. Today it's on its tab. With no tab it reaches needs-you
and nothing else.

## Not decided by #4576

1. **A window with no pane.** After D8 a project can have an empty strip, so `+` has no focused
   pane and a sidebar press nothing to replace. Something must always exist, or both must make
   one. This is the first thing the upgrade hits.
2. **Where a sidebar-only chat's blocking ask goes.** D7 pins a consent open "on the pane". A chat
   in no pane has none. Needs-you covers it, but D7 reads as if it's always visible.
3. **Shells and terminal editors.** Tab, focused pane, or sidebar. A shell can't be found by task
   lineage, and an editor replacing what you're reading is a surprise.
4. **What marks a cross-workspace handoff's arrival**, given no tab.
5. **Whether a chat ending clears its pane**, or removes it as today.
6. **Is a layout saved now?** D6 and D8 imply yes. The record has nothing for it.
7. **A chat replaced out of a pane by D1, with tasks shown in it.** Today tasks live in their
   session's pane. D1 drops lineage as the home, so a press on a task replaces the focused pane,
   which contradicts `CONTEXT.md:849`. Is that intended?

## What I couldn't determine

- **How a sidebar row's menu gets its rows.** I didn't trace it. `TaskEnd.window.test.tsx`
  shows it holds rows for tasks. Whether a plain chat's menu is fed by the tab-keyed verbs is
  unconfirmed, and D9's cost depends on it.
- **The real count of tests that fix this model.** 16 test files name `openTab`, `openTabBehind`,
  `switchTabTo` or `homeOf`. Others assert the wording. I ran nothing.
- **The two-window case.** I read `windows.rs` only for moves. A chat shown in a split window,
  and `in_front` per window, are unchecked (open on the map too).
- **`tabs.ts` below line 1760, and `StripTablist`, `tabRest`, `tabKeys`, `tabSequence`.** Read in
  part or not at all. Keyboard meaning is open on the map, so I left it.
- **Remote or runner chats** (`moveChat`, #2048). Not traced beyond the verb being tab-keyed.
- **Site docs** (`site/`): one hit in `index.mdx`, not read.
- **`looks_at`'s inputs.** Read its use, not how `looked_at` is built, so "must learn panes" is a
  reading of the name and call sites.

## Sources

- #4580, #4575, #4576
- `app/src/tabs.ts`, `PlaneView.tsx`, `actions.ts`, `LayoutPanes.tsx`, `Notice.tsx`, `Views.tsx`,
  `tabChats.ts`, `chatsScope.ts`, `browsers.ts`, `taskKeys.ts`, `settings/links.ts`
- `app/src-tauri/src/`: `chats.rs`, `lib.rs`, `handoff.rs`, `asknotify.rs`, `windows.rs`,
  `plugin/skills/*/SKILL.md`
- `crates/purlis-core/src/`: `reopen.rs`, `handoff.rs`, `dispatch*.rs`; `crates/purlis-cli/src/`:
  `handoff.rs`, `session.rs`
- `crates/purlis-core/docs/handoff.md`, `CONTEXT.md`, `docs/spec.md`
