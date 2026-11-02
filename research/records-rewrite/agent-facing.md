# Agent-facing text that says or assumes a chat has a tab

Read at `research-records-rewrite` = origin/main `421410a1b`, read-only. Nothing was built or run.
Rulings: `rulings.md` beside this file (D1-D9, #4588 1-8, #4587, #4585, #4582, #4584, #4583, #4701).

## Counts

| Group | Must change | Optional | Kept on purpose |
|---|---|---|---|
| (a) skills (12 shipped, not 13: see below) | 14 | 1 | 4 |
| (b) agent docs: `docs/agents/`, `AGENTS.md`, `CLAUDE.md` | 0 | 0 | none |
| (b) docs that `purlis docs show` serves to agents | 20 (b11 is one table of 29 handoff.md lines) | 1 | about 20 view-tab lines |
| (c) Rust/CLI strings sent to agents | 45 (c1-c47, less c10 moved to (e) and c41 optional) | 1 (`opened chat N`) | about 30 view-tab strings |
| (d) tests and fixtures that pin (c) wording | 28 table rows in 20 files, plus 4 fixture rows | | |
| (e) appendix: strings the person sees but no agent reads, found on the way | 17 | | |

**There are 12 skills, not 13.** `app/src-tauri/plugin/skills/` holds add-curation-action, browser,
building-purlis-ui, compact, handoff, persona, safe-remove, secrets, smart-close, theme, update and
working-in-a-clone. None has a `references/` folder. `plugin/hooks/hooks.json`,
`plugin/.claude-plugin/plugin.json` and `plugin/opencode/purlis.ts` have no agent-facing tab text.
The `.ts` file has only code comments.

## Words used throughout, so every surface says the same thing

The rulings split what used to sit "on the chat's tab" into two kinds:

- **An ask** is anything that blocks until the person decides. `CONTEXT.md` "Ask" lists them: a
  permission, a dispatch grant, a host or folder the sandbox refused, workspace removal, a proposed
  change. #4588 rule 5 puts an ask from a chat in no pane on its row, the rows above it, the ✋ and
  the chip count, and the person answers it in the Inbox. If the chat is on screen, its pane band
  shows the ask too. The phrase for this, used everywhere below:
  - **ASK-HERE:** `in the Inbox, and on this chat's pane when it is on screen`
  - **ASK-THERE:** `in the Inbox, and on its pane when it is on screen`
- **A note** is everything else: MCP Retry, plugin-off reason, quiet task, a pin that moved, a
  read-back block line, purlis's own block with Report. **The rulings don't say where a note from a
  chat in no pane goes** (#4584 rules only on asks). So the new wording below never says where a
  note is. It says the person "has a Notice about it", and stays true whatever is decided.
  **OPEN-1.**

Other fixed phrases:

- A task the person started with Ask <persona>… (#1438): **"from `x`"** / **"from this chat"**. Not
  "from the tab of". The action now lives on the chat (D9).
- Where the person acts on a chat: **"its ⋯ menu"**. #4585: the pane head's ⋯, the sidebar row's ⋯
  and the palette read one list.
- A task's conversation: **"the person's, to read in the window"**. A task starts in no pane (D4).
- Smart close: **"ends this chat"** (#4585 rule 5: it ends like End, and the pane keeps the last
  screen with Resume and Clear pane).
- A handoff arriving (#4588 rule 6): **"It starts in that workspace's sidebar and takes no pane. The
  person is told it arrived: a card on that workspace's chip, an Update in the Inbox, and a "new"
  mark on its row."**

**Recommendation (not in the rulings):** make ASK-HERE one constant, for example
`purlis_core::asks::WHERE_ASKED`, the way `handoff::ONE_ROUTE` is one. Add an integration test like
`crates/purlis-core/tests/it/the_one_route_is_taught_in_the_same_words.rs` that holds the handoff,
persona, secrets and safe-remove skills, and `docs/handoff.md`, to it word for word. Then the
sixteen places below that say ASK-HERE can't drift apart.

---

## (a) Skills shipped to chats

### a1. `app/src-tauri/plugin/skills/purlis-handoff/SKILL.md:26-28`
Current: "**2. Will the operator read the new chat themselves?** Then it is a **handoff**: a bug to
fix, a chore, a question for another workspace to own. It gets a tab of its own and this chat
hears nothing back."
New:
> **2. Will the operator read the new chat themselves?** Then it is a **handoff**: a bug to
> fix, a chore, a question for another workspace to own. It starts in the sidebar without taking a
> pane, and the operator is told it arrived. This chat hears nothing back.

Change with: b1 (`handoff.md:35`).

### a2. `purlis-handoff/SKILL.md:70`
Current: "Make the first line a title somebody scanning a strip of tabs would recognise."
New:
> Make the first line a title somebody scanning the Chats list would recognise.

### a3. `purlis-handoff/SKILL.md:112-115`
Current: "**Always pass `--name`, first**: a short task name you write from the brief, a few words a
person scanning a strip of tabs recognises, such as `drop account-console-commons`, not `handoff`
and not the workspace's name. At most 64 characters, plain text. It is what the new chat's tab
says; without it the tab says `<persona> <N>`, and four handoffs look alike."
New:
> **Always pass `--name`, first**: a short task name you write from the brief, a few words a person
> scanning the Chats list recognises, such as `drop account-console-commons`, not `handoff` and
> not the workspace's name. At most 64 characters, plain text. It is what the new chat's row, its
> pane head and the card that says it arrived all show. Without it they say `<persona> <N>`, and
> four handoffs look alike.

Change with: b2 (`handoff.md:58`).

### a4. `purlis-handoff/SKILL.md:133-134`
Current: "- **`opened chat N in workspace '…'`**: the new chat is a tab in that workspace, already
started on the brief. To this chat's own persona, that is the answer, except on a pool's runner."
New:
> - **`opened chat N in workspace '…'`**: the new chat runs in that workspace, already started on
>   the brief. It is in that workspace's sidebar and takes no pane. The operator is told it arrived:
>   a card on that workspace's chip, an Update in the Inbox, and a "new" mark on its row. To this
>   chat's own persona, that is the answer, except on a pool's runner.

This quotes c41 (`purlis-cli/src/handoff.rs:305`). If c41 is ever reworded, this line changes with it.

### a5. `purlis-handoff/SKILL.md:135-136`
Current: "- **`held for the person`**: the handoff is to another persona, and nobody has yet allowed
this pair. purlis is asking the operator, on this chat's tab, with the brief in front of them."
New:
> - **`held for the person`**: the handoff is to another persona, and nobody has yet allowed this
>   pair. purlis is asking the operator in the Inbox, and on this chat's pane when it is on screen,
>   with the brief in front of them.

Change with: c32 (the CLI sentence it paraphrases).

### a6. `purlis-handoff/SKILL.md:181-182`
Current: "- **Nobody approves the brief.** The chat that receives it is told who asked, and every
command of its that asks the operator still asks, in its own tab."
New:
> - **Nobody approves the brief.** The chat that receives it is told who asked, and every command
>   of its that asks the operator still asks them, in the Inbox and on its pane when it is on
>   screen.

### a7. `purlis-persona/SKILL.md:55-57`
Current: "In a chat purlis started, `purlis persona use` is refused and says the two ways forward:
the operator allows the vault on this chat's tab, or you dispatch to the persona that owns the
work."
New:
> In a chat purlis started, `purlis persona use` is refused and says the two ways forward: the
> operator allows the vault in the Inbox, or on this chat's pane when it is on screen, or you
> dispatch to the persona that owns the work.

**OPEN-2:** a vault refusal is not an Inbox ask today. `bindings.ts` `AskSource` has no vault
source, and the Allow lives only in `VaultRefusedNotice`, a pane notice. So for a chat in no pane
the Allow has nowhere to show, and this wording would be false. The rulings' intent (#4588 rule 5:
a consent from a chat in no pane is answered in the Inbox) means making it an ask. a7, a11, b12,
b16, c17, c20 and c21 assume that. If it isn't done, drop "in the Inbox" from all seven together.

Change with: c21 (the refusal the skill describes).

### a8. `purlis-secrets/SKILL.md:121-122`
Current: "Tell the user the host purlis named: a notice on this chat's tab lets them allow it."
New:
> Tell the user the host purlis named: a Notice in the Inbox, and on this chat's pane when it is on
> screen, lets them allow it.

Change with: c18.

### a9. `purlis-secrets/SKILL.md:126-128`
Current: "…when it may not, purlis says so and the tab offers Allow, as above. Do not rewrite the
host or the port yourself."
New:
> …when it may not, purlis says so and the user is offered Allow, as above. Do not rewrite the host
> or the port yourself.

### a10. `purlis-secrets/SKILL.md:129-131`
Current: "**ssh, and git over ssh,** need no vault: run them as you would, and they reach a host this
chat may reach; one it may not is refused with the same Allow on the tab. Do not add a
`ProxyCommand` or `-F` of your own."
New:
> **ssh, and git over ssh,** need no vault: run them as you would, and they reach a host this chat
> may reach. One it may not reach is refused, with the same Allow. Do not add a `ProxyCommand` or
> `-F` of your own.

### a11. `purlis-secrets/SKILL.md:133-134`
Current: "**When a vault is refused for this chat's persona**, the refusal names the ways forward. A
notice on this chat's tab lets the user press *Allow*; once they have, purlis tells this chat what
now runs…"
New:
> **When a vault is refused for this chat's persona**, the refusal names the ways forward. A notice
> in the Inbox, and on this chat's pane when it is on screen, lets the user press *Allow*. Once they
> have, purlis tells this chat what now runs…

OPEN-2 applies. Change with: c17 and c20.

### a12. `purlis-safe-remove/SKILL.md:73-77`
Current: "- `purlis workspace remove <name>` hands the name to purlis, which asks the operator on
this chat's tab: **Remove…** or **Keep**. … Tell the operator the question is waiting on this
chat's tab, then end your turn."
New:
> - `purlis workspace remove <name>` hands the name to purlis, which asks the operator in the Inbox,
>   and on this chat's pane when it is on screen: **Remove…** or **Keep**. Remove… opens the window's
>   Delete workspace, and the operator's press there is what removes. The command says the operator
>   has been asked and exits without removing anything. That is not a failure. Tell the operator the
>   question is waiting in the Inbox, then end your turn.

(The ask source `workspace-removal` is already an Inbox ask, so nothing else has to change for this
to be true.)

### a13. `purlis-smart-close/SKILL.md:9-10`
Current: "The last command below is what closes the tab, so run it last, once."
New:
> The last command below is what ends this chat, so run it last, once.

### a14. `purlis-smart-close/SKILL.md:77-80`
Current: "Then stop: when the operator started this close, the app closes this tab once this turn
ends. When the answer says the tab will not close by itself, tell the operator the record is
written and the tab is theirs to close."
New:
> Then stop: when the operator started this close, the app ends this chat once this turn ends, and
> its pane keeps the last screen. When the answer says the chat will not end by itself, tell the
> operator the record is written and the chat is theirs to end.

Change with: c26 and c35-c40 (the answers this skill reads).

### a15 (optional). `purlis-theme/SKILL.md:148-149`, `:189`
Current: "the session tabs (chats, views, files) and their `×`" and "**The session tabs** are the
`tabs` family."
New (148-149): "the session tabs (each holds panes of chats, views or files) and their `×`". Line 189
can stay. Tabs still exist (D3), so the theme's `tabs.*` family is still right. This is accuracy,
not an instruction that misleads.

**Kept:** `purlis-secrets/SKILL.md:39` and `:55` ("the vault's tab"): a vault is a view, and views
keep tabs (D3). `purlis-theme/SKILL.md:72-73, 123-129, 190-197` are theme parts.
`purlis-building-purlis-ui/SKILL.md:42` names the UI `tabs` component family.

---

## (b) Agent docs

**`docs/agents/` (domain, issue-tracker, triage-labels, skills/landing-a-pr), `AGENTS.md`,
`CLAUDE.md`: nothing.** The only hits are "table".

**But `crates/purlis-core/docs/*.md` is agent-facing.** `purlis docs show <page>` serves these pages,
and the handoff skill sends chats there ("`purlis docs show handoff` has the whole of it"). The
rulings hand `handoff.md` to "Rewriting the records" (#4704), so whoever does that rewrite owns
these lines. They are listed here because an agent reads them as instructions.

### b1. `handoff.md:34-35`
Current: "**Will you read the new chat yourself?** Then it is a handoff: a chat that works on its own,
with a tab of its own, and this chat hears nothing back."
New:
> **Will you read the new chat yourself?** Then it is a handoff: a chat that works on its own. It
> starts in the sidebar without taking a pane, you are told it arrived, and this chat hears nothing
> back.

### b2. `handoff.md:58-59`
Current: "- **`--name` is what the new chat is called** — its tab, and wherever else a chat's name is
shown:"
New:
> - **`--name` is what the new chat is called**: its row, its pane head, the card that says it
>   arrived, and wherever else a chat's name is shown:

### b3. `handoff.md:96-98` (step 5)
Current: "5. The chat lands as a new tab on the target workspace's strip, **behind the tab you are
reading**, and the window is not raised. It takes the front only in a window with no tab at all,
where there is nothing to interrupt."
New:
> 5. The chat lands in the target workspace's sidebar and **takes no pane**, and the window is not
>    raised. Three things say it arrived. A card hangs from that workspace's title-bar chip, "<chat>
>    arrived from <asking chat>", with Go to it. It never takes focus, and pressing elsewhere leaves
>    a dot on the chip. An Update goes in the Inbox, and it isn't counted on the ✋. Its row keeps a
>    "new" mark until you have looked at the chat. With no chip, the card is listed in the Notices
>    centre with a way to open the workspace.

### b4. `handoff.md:113-115`
Current: "…the start. There is no mark on the strip beyond the new tab itself: the tab is how you see
it."
New:
> …the start. The chip's card, the Inbox Update and the row's "new" mark are how you see it.

### b5. `handoff.md:121-123`
Current: "That is why a handed-off chat always lands as a tab you can see, stamped with the chat it
came from, and why what consents to it is the app's own decision and never a word the chat sends."
New:
> That is why a handed-off chat always lands where you can see it: announced on its workspace's
> chip and in the Inbox, marked new on its row, and stamped with the chat it came from. And that is
> why what consents to it is the app's own decision, never a word the chat sends.

(#4588 rule 6 carries this safety property over from the tab.)

### b6. `handoff.md:162-163`
Current: "The same note is on the new chat's tab, as its tooltip, and in its pane's corner:"
New:
> The same note is on the new chat's sidebar row, as its tooltip, and on its pane head when it is
> on screen:

**OPEN-3:** #4585 sets the pane head's parts ("harness · workspace") and doesn't name a "from"
note. Whether the head keeps one is up to the build.

### b7. `handoff.md:992-994`
Current: "- **Its first message says you asked**, and from which chat's tab: ``⟨the person asks,
from the tab of `steward 3` · workspace alpha · 2026-10-07 14:32⟩``."
New:
> - **Its first message says you asked**, and from which chat: ``⟨the person asks, from `steward 3`
>   · workspace alpha · 2026-10-07 14:32⟩``.

Change with: c1 and its tests (d1).

### b8. `handoff.md:1106-1107`
Current: "The work moves, the new chat opens as a tab, and it owes no report."
New:
> The work moves, the new chat starts in the sidebar, and it owes no report.

### b9. `handoff.md:1114-1116`
Current: "…purlis holds it and raises a Notice on the asking chat's tab, with **that first brief in
full**."
New:
> …purlis holds it and asks you, in the Inbox and on the asking chat's pane when it is on screen,
> with **that first brief in full**.

### b10. `handoff.md:1131-1133`
Current: "…every command of its that asks you still asks you, in its own tab. The brief is the first
message in that tab, it is kept in the dispatch's record…"
New:
> …every command of its that asks you still asks you, in the Inbox and on its pane when it is on
> screen. The brief is the chat's first message, it is kept in the dispatch's record…

### b11. `handoff.md`, the other tab lines (word swaps; the same owner as b1-b10)

| Line | Current | New sentence |
|---|---|---|
| 179 | "the window shows it as a task of this chat, not as a tab that moved away;" | "the window lists it as a task of this chat;" |
| 242 | "(a task you started from a tab, …)" | "(a task you started from a chat, …)" |
| 269-270 | "**Closing a task's tab ends nothing**: the tab goes, and the task goes back to the Chats list and keeps working." | "**Closing a task's pane ends nothing**: the pane clears, and the task keeps working in the sidebar." |
| 275-276 | "It is on the session's row menu and its tab menu, beside Stop… and on the last line of the tab's task menu, under "End a task"." | "It is on the session's ⋯ menu, on its pane head and its sidebar row, and in the palette, beside Stop with everything below it." (#4585: one list on three surfaces. The tab's task menu goes: #4585's "38-41 go to Keyboard".) |
| 299 | "You can still ask a persona from its tab yourself." | "You can still ask a persona from its ⋯ menu yourself." |
| 331 | "A Notice on the asking chat's tab says who wants to dispatch to whom and shows the brief," | "You are asked in the Inbox, and on the asking chat's pane when it is on screen, who wants to dispatch to whom, with the brief," |
| 351 | "…by its name and what it is doing, and has a tab once you open it." | "…by its name and what it is doing, and takes a pane only when you pick it." |
| 368 | "with nothing to allow on its tab: the grant was your consent to the pair." | "with nothing more for you to allow: the grant was your consent to the pair." |
| 393 | "until you allow them on its tab" | "until you allow them" |
| 443 | "the person starts such a chat from the asking chat's tab." | "the person starts such a chat from the asking chat's ⋯ menu." |
| 580 | "and the asking chat's tab says it to you, by every task's name," | "and the asking chat says it to you in a Notice, by every task's name," |
| 660, 665, 669 | "Its tab's menu ends with how many of its tasks run…", "the number its tab menu counts against", "said on its tab menu's footer too" | **OPEN-4.** #4585 drops the tab's task menu and puts chat rows in one ⋯ list, but doesn't say where the "4 of 6 running" count goes. Suggestion: "Its ⋯ menu ends with how many of its tasks run against the limit in force for it: `4 of 6 running`." |
| 686 | "**A task you asked for yourself, from a chat's tab, does not hold that chat's end.**" | "**A task you asked for yourself, from a chat, does not hold that chat's end.**" |
| 760 | "A task you started yourself from this chat's tab (*Ask <persona>…*) is listed too," | "A task you started yourself from this chat (*Ask <persona>…*) is listed too," |
| 790 | "A wait on a task whose tab you closed says how it ended." | "A wait on a task you ended says how it ended." |
| 830 | "to add to an answer you sent, type in the task's own tab." | "to add to an answer you sent, pick the task and type in its pane." |
| 840 | "A handoff, a task you started from a tab, and your own chats ask you directly," | "A handoff, a task you started from a chat, and your own chats ask you directly," |
| 850 | "whatever the task's tab is showing," | "whatever the task's pane is showing," |
| 882 | "a chat started again in that tab cannot report for the task." | "a chat started again in that pane cannot report for the task." |
| 899 | "a persona chat you started yourself with Ask from a tab" | "a persona chat you started yourself with Ask…" |
| 909-910 | "It resumes the task's conversation in a tab of its own." | "It resumes the task's conversation as a chat of its own, in the sidebar." **OPEN-5:** whether Reopen takes the focused pane. It is the person's press, so D1 suggests yes. |
| 925 | "If the chat whose tab you asked from has closed," | "If the chat you asked from has ended," |
| 928 | "(a permission prompt, a question, in its own tab)" | "(a permission prompt or a question, in the Inbox)" |
| 958 | "A task the person asked for from a tab is answered in the dialog they asked from," | "A task the person asked for from a chat is answered in the dialog they asked from," |
| 980-982 | "**Ask <persona>…** on a chat's tab menu and in the palette, … under the chat whose tab you used." | "**Ask <persona>…** on a chat's ⋯ menu and in the palette, … under the chat you asked from." |
| 1000 | "the Notice on that chat's tab has *Dispatch to <persona>…*," | "the vault's Notice for that chat has *Dispatch to <persona>…*," |
| 1005-1007 | "the entry is not on any tab, … *Ask devops…* is not on the tab of a chat running as `steward`, … asking from that tab would send…" | "the entry is on no chat's menu, … *Ask devops…* is not on the menu of a chat running as `steward`, … asking from that chat would send…" |
| 1313 | "nothing appears on the chat's tab," | "nothing is asked of you," |
| 1344 | "Every command of its that asks still asks, in its own tab," | "Every command of its that asks still asks, in the Inbox and on its pane when it is on screen," |

Kept: 343, 485, 498, 540-575, 586 (Changes tab and Dispatches tab are views); 821, 835 (Activity
tab, a view); 832 ("line breaks and tabs", the character).

### b12. `personas.md:144-146`
Current: "…run the command that needs it and ask the operator to allow the vault on the chat's tab."
New:
> …run the command that needs it and ask the operator to allow the vault, in the Inbox or on the
> chat's pane when it is on screen.

OPEN-2.

### b13. `personas.md:204-209`
Current: "It is told that only the person answers it, in the task's own tab, so that it says so where
the person is; nothing it does answers the prompt. The window says the same on the session's tab:
its chip wears the hand from the moment the prompt is held, and its pane names the task, with
**Show the task**…"
New:
> It is told that only the person answers it, from the Inbox, so that it says so where the person
> is. Nothing it does answers the prompt. The window says the same on the session's row: it wears
> the hand from the moment the prompt is held, and the session's pane, when it is on screen, names
> the task, with **Show the task**…

**OPEN-6:** `CONTEXT.md` "Where you answer" (the setting from the 2026-11-02 grill: "from the top,
in the tab of the nearest chat above that has one, or from each session, in the task's own tab")
is defined in tabs. None of the rulings redefine it. The wording above, b11 line 830, c9 and c13
avoid it by saying "the Inbox", which is always true. The setting itself needs a ruling.

### b14. `personas.md:356`
Current: "When the second fails too, the chat's tab shows a Notice naming the server…"
New:
> When the second fails too, the person has a Notice naming the server…

OPEN-1. Change with: c11.

### b15. `secrets.md:60-63`
Current: "…the chat's tab shows the same sandbox block Notice as for the chat's own command, … purlis
says on stderr that its sandbox refused that host and that it can be allowed on the tab."
New:
> …the person is asked about it with the same sandbox block Notice as for the chat's own command,
> in the Inbox and on the chat's pane when it is on screen, … purlis says on stderr that its sandbox
> refused that host and that the person can allow it.

Change with: c18.

### b16. `secrets.md:110-111`
Current: "…refused with a sentence naming the ways forward, and the chat's tab shows a notice with
**Allow {persona} to use this vault** and **Keep blocked**."
New:
> …refused with a sentence naming the ways forward, and the person has a notice, in the Inbox and on
> the chat's pane when it is on screen, with **Allow {persona} to use this vault** and **Keep
> blocked**.

OPEN-2.

### b17. `hooks.md:407-408`
Current: "Your own terminal, a harness you start yourself, and a shell tab are never held to it."
New:
> Your own terminal, a harness you start yourself, and a purlis shell are never held to it.

### b18. `hooks.md:947-948`
Current: "a chat running on instructions that changed after it started is marked on its tab in the
window instead."
New:
> a chat running on instructions that changed after it started is marked on its sidebar row and
> its pane head instead.

### b19. `control-plane.md:560-561`
Current: "**Sign in**, on its row in Settings or on its Ask in the Inbox, opens a shell tab that runs
`claude auth login`…"
New:
> **Sign in**, on its row in Settings or on its Ask in the Inbox, opens a shell that runs `claude
> auth login`…

**OPEN-7:** #4588 rule 8 puts a forge login typed into a shell in a one-job split beside the
focused pane. It doesn't say where an Inbox or Settings Sign in opens: as a one-job split, or a new
shell taking the focused pane. "opens a shell" holds either way. The same applies to c45, c46, c47
and e-items 9-10.

### b20. `control-plane.md:586-592`
Current: "…its pin moving and the chat's tab saying so; … its pin moves with the version installed,
and the chat's tab says so."
New: "…its pin moving and the chat saying so in a Notice; … its pin moves with the version installed,
and the chat says so in a Notice." (OPEN-1.)

Optional: `secrets.md:349` "however many chats and tabs ask" → "however many chats ask".
Kept: `secrets.md:173, 450-606` (vault tab, a view). `control-plane.md:577` (the persona's tab, a
view). `themes.md` (theme parts). `workspaces.md:168` (the character).

---

## (c) Rust and CLI strings sent to agents

All strings are Rust string literals. A trailing `\` is a source line break and doesn't show in
the text. Paths are relative to the worktree.

### The stamp and note of a task the person started (#1438)

**c1. `crates/purlis-core/src/handoff.rs:487-488` `PERSON_TASK_STAMP`** (every such task's first line)
Current: `"⟨the person asks, from the tab of `{from}` · {place} · {when}⟩"`
New: `"⟨the person asks, from `{from}` · {place} · {when}⟩"`
It still opens with words no chat's stamp opens with, so the forgery guard holds. Fix the doc
comments at 481-482 and 504 ("from a chat's tab", "from the tab of chat `on`") to "from a chat" and
"from chat `on`".
Change with: d1, d2, b7, and the UI facts line in `app/src/Brief.window.test.tsx:403` (e-17).

**c2. `handoff.rs:496-497` `PERSON_TASK_NOTE`**
Current: `"⟨the person typed the request below in purlis's own window, on that chat's tab. Your own persona's rules apply, and every command that asks the person still asks them. …"`
New: `"⟨the person typed the request below in purlis's own window, asking from that chat. Your own persona's rules apply, and every command that asks the person still asks them. …"` (the rest unchanged).
Also fix the doc comment at 493-494 ("typed into purlis's dialog on that chat's tab", "in this chat's
own tab"). No test pins the changed words: `handoff.rs:1257-1261` checks other parts.

**c3. `app/src-tauri/src/rebrief.rs:147-150` `PERSON_STAMP`**
Current: `"⟨purlis started this chat again with no conversation. It was dispatched, and below is what the person asked then from the tab of `{asker}`, read from purlis's own record of the dispatch: …⟩"`
New: `"⟨purlis started this chat again with no conversation. It was dispatched, and below is what the person asked then from `{asker}`, read from purlis's own record of the dispatch: the same request, not a new one, and not where its first turn left off⟩"`
Its test (`rebrief/tests.rs:117`) builds from the constant, so nothing to edit.

### Telling a chat about a task the person started

**c4. `crates/purlis-core/src/dispatched.rs:360-363` `asked_by_the_person`**
Current: `"the person asked for chat {of} themselves, from this chat's tab, so it is theirs and not this chat's to wait on, tell, answer or cancel. Nothing was sent. Its report comes to this chat when it has one."`
New: `"the person asked for chat {of} themselves, from this chat, so it is theirs and not this chat's to wait on, tell, answer or cancel. Nothing was sent. Its report comes to this chat when it has one."`
Change with: d3.

**c5. `dispatched.rs:403-405` `the_person_typed_in`**
Current: `"the person has typed in '{name}' (chat {of}), so it is theirs to {act}, from its tab."`
New: `"the person has typed in '{name}' (chat {of}), so it is theirs to {act}, from its ⋯ menu."`
Change with: d4.

**c6. `dispatched.rs:728-738` `output_text`** (two strings, plus the doc comment at 731)
Current: "…The whole conversation is the person's, in its tab." (twice)
New: "…The whole conversation is the person's, to read in the window." (both)
Change with: d5, c27, c28.

**c7. `dispatched.rs:2252-2254` `STARTED_BY_THE_PERSON`**
Current: `" · started by the person from this chat's tab: its report comes here, and it is not this chat's to wait on, tell, answer or cancel"`
New: `" · started by the person from this chat: its report comes here, and it is not this chat's to wait on, tell, answer or cancel"`
Change with: d6 (the CLI test spells it out).

**c8. `crates/purlis-core/src/dispatchtalk.rs:158-162` `ASKED_BY_THE_PERSON`**
Current: `"the person started this task from another chat's tab, and that chat did not dispatch it: it reads this task's report and nothing else, and cannot answer. …"`
New: `"the person started this task from another chat, and that chat did not dispatch it: it reads this task's report and nothing else, and cannot answer. Nothing was sent. A question for the person is asked in this chat itself, and everything else goes in the report."`
Its tests use the constant.

**c9. `dispatchtalk.rs:1849-1855` `no_question`** (doc comment at 1850 too)
Current: `"… A question it has put to the person is the person's to answer, in its own tab: no chat can answer it."`
New: `"'{name}' (chat {task}) has asked this chat no question that is still open, so there is nothing here to answer. A question it has put to the person is the person's to answer, from the Inbox: no chat can answer it."`
Change with: d7, d8. OPEN-6.

**c10.** Moved to (e): `not_the_person_s_to_answer` is read by the person, not an agent.

**c11. `dispatchtalk.rs:2493-2500` `mcp_said`**
Current: `"… The person has a Notice on this chat's tab with Retry, which restarts the chat on this conversation. …"`
New: `"⬢ purlis: this chat's MCP server `{}` could not start, so its tools are not here. purlis started it twice as the chat started, and both failed. The person has a Notice about it with Retry, which restarts the chat on this conversation. Do not try to reach the server another way; say so if the work needs its tools. …"`
OPEN-1. Change with: d9, b14.

**c12. `crates/purlis-core/src/handback.rs` `whose`, three copies: 1033-1038, 1080-1086, 1270-1276** (and the comment at 1031)
Current: `"a task the person started from the tab of `{}`, a chat {where_it_was} that has since closed"` and `"a task the person started from this chat's tab, which you did not dispatch"`
New: `"a task the person started from `{}`, a chat {where_it_was} that has since ended"` and `"a task the person started from this chat, which you did not dispatch"`
"Closed" becomes "ended" because D2 makes close mean clearing a pane. **The sibling arms,
`"the task `{}` — a chat {where_it_was} that has since closed — dispatched to it"`, at about 1040,
1077 and 1267, must say "ended" in the same change, or the three arms read differently.**
Change with: d10, d11, d12, d13.

**c13. `crates/purlis-core/src/awareness.rs:946-949`**
Current: `"⬢ A task of yours is stopped ({AS_DATA}): {}. Only the person answers it, in that task's own tab: tell them, and do not answer it or work around it."`
New: `"⬢ A task of yours is stopped ({AS_DATA}): {}. Only the person answers it, from the Inbox: tell them, and do not answer it or work around it."`
It's true for every prompt kind: a terminal prompt is listed in the Inbox with Go to chat (#4588
rule 5's check). OPEN-6. Change with: d14.

### Sandbox, vaults, hosts

**c14. `crates/purlis-core/src/sandboxblock.rs:424-436`** (two strings)
Current (all-per-user branch): `"⬢ purlis's sandbox blocked {}. The person has a Notice about it on this chat's tab.{way_round}"`
New: `"⬢ purlis's sandbox blocked {}. The person has a Notice about it.{way_round}"` (OPEN-1: no grant is possible, so this is a note, not an ask.)
Current (main branch): `"⬢ purlis's sandbox blocked {}. The person has a Notice about it on this chat's tab, with what they can do about it.{ours} Say in a line …"`
New: `"⬢ purlis's sandbox blocked {}. The person is asked about it in the Inbox, and on this chat's pane when it is on screen, with what they can do about it.{ours} Say in a line what was blocked and what you need it for, then wait for their answer: do not work around the block (another host, another tool, another way in) and do not retry before they answer. If they allow it, this chat is started again on this conversation and told to retry.{way_round}"`
Change with: d15. The test's word list must swap "this chat's tab" for "in the Inbox".

**c15. `sandboxblock.rs:455-461` `told_the_chat_of_a_read`**
Current: `"… The person has a quiet line about it on this chat's tab, and nothing waits on their answer, so carry on. …"`
New: `"⬢ What you just read holds the words of a sandbox refusal: {}. The person has a quiet line about it, and nothing waits on their answer, so carry on. If a command of yours needs what was refused, say so in a line and run it again where its result comes back to you: a refusal of a command you run raises a Notice the person can answer. Do not work around the block."`
OPEN-1. No test pins "tab" here (checked `sandboxblock/tests.rs`).

**c16. `crates/purlis-core/src/briefing.rs:244-246` `SANDBOXED_NOTE`** (SessionStart briefing in every sandboxed chat)
Current: `"⬢ **This chat runs in purlis's sandbox.** When it blocks a command (a host it may not reach, or a folder it may not write), the person sees a Notice on this chat's tab and chooses **Allow for this chat**, …"`
New: `"⬢ **This chat runs in purlis's sandbox.** When it blocks a command (a host it may not reach, or a folder it may not write), the person is asked in the Inbox, and on this chat's pane when it is on screen, and chooses **Allow for this chat**, **Always allow** (…) or **Keep blocked**. …"` (the rest unchanged)
`briefing/tests.rs:1705-1717` lists other words, so no test edit is needed.

**c17. `crates/purlis-core/src/secrets/brokered.rs:789-790` `ALLOW`**
Current: `"ask the operator to press Allow in the notice on this chat's tab, then run the command again (no restart is needed)"`
New: `"ask the operator to press Allow on the vault's notice, in the Inbox or on this chat's pane when it is on screen, then run the command again (no restart is needed)"`
OPEN-2. Change with: d16.

**c18. `brokered.rs:1475-1481` `refused_note`**
Current: `"… The person can allow it on this chat's tab (Allow for this chat, or Always allow); the chat is then started again with it, and the command can be run again."`
New: `"purlis's sandbox refused this command a connection to {target}, which this chat's hosts do not list. It is the sandbox, not the server. The person can allow it in the Inbox, or on this chat's pane when it is on screen (Allow for this chat, or Always allow); the chat is then started again with it, and the command can be run again."`
Change with: a8, b15. No test pins the "tab" words (checked).

**c19. `brokered.rs:1495-1498`**
Current: `"… purlis keeps only the first {REFUSALS_KEPT} it refuses, so these are neither named nor offered on the chat's tab."`
New: `"purlis's sandbox also refused this command connections to {hosts} that this chat's hosts do not list. purlis keeps only the first {REFUSALS_KEPT} it refuses, so these are neither named nor offered to the person."`

**c20. `crates/purlis-core/src/secrets/vaultcmd.rs:829-832` `NOT_ALLOWED_ROUTES`**
Current: `"For a vault marked not allowed: run the command that needs it, and ask the operator to press Allow in the notice on this chat's tab; or, …"`
New: `"For a vault marked not allowed: run the command that needs it, and ask the operator to press Allow on the vault's notice, in the Inbox or on this chat's pane when it is on screen; or, to have the vault's persona do the work, dispatch to it (`purlis dispatch --to <name>`). A chat's persona is fixed for its life."`
OPEN-2. Its tests check `contains("press Allow")`, which still holds.

**c21. `crates/purlis-core/src/personaverbs/select.rs:264-268`**
Current: `"To use a vault of '{name}' here, run the command that needs it and ask the operator to press Allow in the notice on this chat's tab. To have '{name}' do the work, dispatch to it: `purlis dispatch --to {name}`."`
New: `"To use a vault of '{name}' here, run the command that needs it and ask the operator to press Allow on the vault's notice, in the Inbox or on this chat's pane when it is on screen. To have '{name}' do the work, dispatch to it: `purlis dispatch --to {name}`."`
OPEN-2. Change with: d17, the fixture row 244 (d-F4), a7.

### Dispatch grants and refusals

**c22. `crates/purlis-core/src/dispatchplace.rs:202-209`**
Current: `"… The person can start it from this chat's tab. Leave out `--in` and the new chat works in this chat's folder."`
New: `"… The person can start it from this chat's ⋯ menu. Leave out `--in` and the new chat works in this chat's folder."`
Change with: d18, the fixture row 22 (d-F2).

**c23. `crates/purlis-core/src/dispatchdecision.rs:264-266` `HELD`**
Current: `"this chat runs with another persona's grants until the person allows its own, so it cannot dispatch yet. Ask the person to allow this chat its own grants on its tab, then dispatch again."`
New: `"this chat runs with another persona's grants until the person allows its own, so it cannot dispatch yet. Ask the person to allow this chat its own grants, then dispatch again."`
This doesn't say where, because **OPEN-8**: the "allow its own grants" control isn't an Inbox ask
source, and the rulings don't say where it is for a chat in no pane.
Change with: the fixture row 19 (d-F1). Its CLI tests use the constant.

**c24. `app/src-tauri/src/dispatchgrants.rs:272-274` `HELD_DISPATCHES_TO_NO_ONE`**
Current: `"this chat runs on another chat's grants until the person allows its own on its tab, so it dispatches to no one yet. Ask the person to press Allow on this chat's tab."`
New: `"this chat runs on another chat's grants until the person allows its own, so it dispatches to no one yet. Ask the person to allow this chat its own grants."`
OPEN-8. No test spells it out.

**c25. `app/src-tauri/src/dispatchunattended.rs:124-126` `HOLDS_ANOTHERS`** (doc comment at 124 too)
Current: `"… It dispatches to nobody until a person allows them on its tab."`
New: `"this chat still runs on the grants of the chat that opened it, and it runs with its harness's permission prompts off, so nobody is here to allow it its own. It dispatches to nobody until a person allows them."`
Change with: d19.

### MCP tool descriptions (`crates/purlis-core/src/chattools.rs`)

**c26. `chattools.rs:689-694` `SESSION_RECORD`**
Current: `"… purlis writes it where this chat works and says whether the tab now closes, which it does when the person started a smart close. …"`
New: `"Write this chat's session record: a summary of this session, never the transcript, that the next chat reads to pick up where this one stopped. purlis writes it where this chat works and says whether the chat now ends, which it does when the person started a smart close. The body is exactly the sections ## Goal, ## Done, ## Decisions, ## Open and ## How to resume, in that order. Never a secret."`
Change with: a14, c35-c40.

**c27. `chattools.rs:917-920` `DISPATCH_READ`**
Current: `"… Only a task this chat dispatched can be read; the whole conversation is the person's, in the task's tab."`
New: `"Read what a task this chat dispatched said last and the commands it ran, with their exit codes, as its harness handed them to purlis: quoted as data, credential shapes masked. Only a task this chat dispatched can be read; the whole conversation is the person's, to read in the window."`
Also adjacent, in `DISPATCH_LIST` at about 915: "or this chat closes" → "or this chat ends" (see the
close/end note below).

### CLI help text (agents read `--help`)

**c28. `crates/purlis-cli/src/dispatch.rs:137-140`** (the `Read` subcommand's doc)
Current: "…the whole conversation is the person's, in the task's tab."
New: "…the whole conversation is the person's, to read in the window."

**c29. `crates/purlis-cli/src/main.rs:577-580`** (the `Dispatch` doc)
Current: "…where there is none the person is asked on this chat's tab, and the task starts when they
allow it."
New: "…where there is none the person is asked in the Inbox, and on this chat's pane when it is on
screen, and the task starts when they allow it."

**c30. `main.rs:639-641`** (the `Handoff` doc)
Current: "…to this chat's own persona it opens at once, and to another purlis asks you once for the
pair, on this chat's tab."
New: "…to this chat's own persona it starts at once, and to another purlis asks you once for the
pair, in the Inbox and on this chat's pane when it is on screen."

**c31. `main.rs:651-653`**
Current: "**The app opens the chat.** Run from a chat the purlis app started, the new chat opens there
as a tab in the workspace you name."
New: "**The app starts the chat.** Run from a chat the purlis app started, the new chat starts there,
in the sidebar of the workspace you name, and takes no pane. The person is told it arrived."

### CLI results

**c32. `crates/purlis-cli/src/handoff.rs:455-461`** (and its doc comment at 447)
Current: `"{HANDOFF_SAYS} held for the person. {} may not dispatch to '{}' yet, so the person is being asked on this chat's tab, with this brief in front of them. Nothing has been opened. …"`
New: `"{HANDOFF_SAYS} held for the person. {} may not dispatch to '{}' yet, so the person is being asked in the Inbox, and on this chat's pane when it is on screen, with this brief in front of them. Nothing has been opened. If they allow it, the chat opens in workspace '{ws}' then, started on the brief;{created} if they keep it blocked, this chat is told on its next turn. Carry on with other work, and do not hand it off again."`
Change with: d20, a5.

**c33. `crates/purlis-cli/src/dispatch.rs:532-538`** (and its doc comment at 529)
Current: `"{SAYS} held for the person. {} may not dispatch to '{}' yet, so the person is being asked on this chat's tab, with this brief in front of them. Nothing has started. …"`
New: `"{SAYS} held for the person. {} may not dispatch to '{}' yet, so the person is being asked in the Inbox, and on this chat's pane when it is on screen, with this brief in front of them. Nothing has started. If they allow it, '{}' starts then and its report reaches this chat as context on a later turn; if they keep it blocked, this chat is told on its next turn. Carry on with other work, and do not dispatch it again."`
Change with: d21, d22, the fixture row 57 (d-F3).

**c34. `dispatch.rs:697-699`**
Current: `"{REPORT_SAYS} '{}' has closed, and the person started this task from its tab, so the report is theirs: this chat is marked as needing them, and they read it here."`
New: `"{REPORT_SAYS} '{}' has ended, and the person started this task from it, so the report is theirs: this chat is marked as needing them, and they read it here."`
No test spells this out (checked).

**c35. `crates/purlis-cli/src/session.rs:260-264` `NO_PASS`** (the doc comment says "the sentence the skill relays to the operator")
Current: `"No smart close was started for this chat (the tab's Smart close, or typing /purlis-smart-close), so its tab will not close by itself. Close it when you are done."`
New: `"No smart close was started for this chat (Smart close in its End… dialog, or typing /purlis-smart-close), so it will not end by itself. End it when you are done."`
(#4585 rule 5: Smart close is End…'s third answer, wherever End… is.)
Change with: d23, d24, a14.

**c36. `session.rs:302-306`**
Current: `"The app wrote it, and closes this tab when this turn ends."`
New: `"The app wrote it, and ends this chat when this turn ends."`
Change with: d25.

**c37. `session.rs:350-353`**
Current: `"This chat was not started by the app (no $PURLIS_HOOK_SOCKET and $PURLIS_CHAT), so its tab will not close by itself — close it when you are done."`
New: `"This chat was not started by the app (no $PURLIS_HOOK_SOCKET and $PURLIS_CHAT), so nothing ends it by itself. End it when you are done."`
Change with: d23 (the env.len()==1 branch).

**c38. `session.rs:366-368`**
Current: `"Told the app: a Smart close waiting on this record closes the tab."`
New: `"Told the app: a Smart close waiting on this record ends the chat."`

**c39. `session.rs:385-388`**
Current: `"The app did not hear it ({not_heard}), so this chat's hook tells it when this turn ends. If the tab is still open after that, close it yourself."`
New: `"The app did not hear it ({not_heard}), so this chat's hook tells it when this turn ends. If the chat is still running after that, end it yourself."`
Change with: d23 ("close it yourself" becomes "end it yourself").

**c40. `session.rs:391-396`**
Current: `"The app did not hear it ({not_heard}), and purlis could not leave it for this chat's hook ({kept}), so the tab will not close by itself — close it yourself when you are done."`
New: `"The app did not hear it ({not_heard}), and purlis could not leave it for this chat's hook ({kept}), so the chat will not end by itself. End it yourself when you are done."`
Optional internal tidy-up, same change: the field `Saved.tab` (`session.rs:159`, `:240`, `:302`) and
the doc comments at 152, 158-160, 333-335 and `mcp.rs:364` ("what becomes of the tab") become
`ends` / "what becomes of the chat".

**c41 (optional, recommend keeping). `crates/purlis-cli/src/handoff.rs:305`**
Current: `"{HANDOFF_SAYS} opened chat {chat} in workspace '{ws}', started on the brief"`
"Opened" doesn't name a tab, and a4 now explains it. Rewording it to "started chat N" would ripple
into a4, the fixture row 111 (`handoff-inside-the-app-opens-the-chat-there-and-records-its-todo`,
`tests/fixtures/recorded/behaviour.jsonl:111`) and the handoff CLI tests. Recommendation: keep it.

### Other agent-read strings

**c42. `crates/purlis-core/src/doctor/sandbox.rs:392-399`** (`purlis doctor`, which agents run)
Current: `"A block of purlis's own operation is a purlis bug. The chat's tab offers Report, which shows a draft …"`
New: `"A block of purlis's own operation is a purlis bug. Its Notice on the chat offers Report, which shows a draft naming only the operation, the kind of path and the versions, and sends nothing until you press File report."`
OPEN-1. Change with: d26.

**c43. `crates/purlis-core/src/harness_plugin/on_here.rs:93-96`**
Current: `"Only this project's settings turn the {name} plugin on, and this chat does not read them. The chat's tab says why."`
New: `"Only this project's settings turn the {name} plugin on, and this chat does not read them. A Notice on the chat says why."` (OPEN-1.)

**c44. `crates/purlis-core/src/harness_declaration.rs:1081-1084`** (a lint read by whoever writes a declaration, often an agent)
Current: `"{} says terminal = false — every harness purlis runs starts in its terminal (level 1), and a chat in a tab always does. Remove the line."`
New: `"{} says terminal = false — every harness purlis runs starts in its terminal (level 1), and every chat has one, in a pane or not. Remove the line."`

**c45. `crates/purlis-core/src/acp.rs:287-289`** (error)
Current: `"the ACP agent needs a login, which purlis never supplies: log in through the harness's own flow in a shell tab"`
New: `"the ACP agent needs a login, which purlis never supplies: log in through the harness's own flow in a shell"`
Change with: d27. OPEN-7.

**c46. `crates/purlis-core/src/profilestore/sign_in.rs:422-426`** (a start refused for sign-in. A dispatch hits it too, so agents relay it.)
Current: `"… It waits in the Inbox as Sign in {name}, which runs {title}'s own sign-in in a shell tab, or run purlis profile sign-in {name} in your own terminal; then start the chat again."`
New: `"the profile '{name}' is not signed in on {here}. Nothing was started. It waits in the Inbox as Sign in {name}, which runs {title}'s own sign-in in a shell, or run purlis profile sign-in {name} in your own terminal; then start the chat again."`
OPEN-7.

**c47. `crates/purlis-cli/src/main.rs:4484-4489`** (CLI refusal)
Current: `"… Sign '{}' in from its line in Settings › Pools, with a project open: its Sign in opens a shell tab on the runner."`
New: `"purlis follows only Claude Code's and opencode's Copilot sign-in on a pool so far. Sign '{}' in from its line in Settings › Pools, with a project open: its Sign in opens a shell on the runner."`
OPEN-7.

**Kept on purpose (views keep tabs, D3, or not a chat tab):** every "vault's tab" (`secrets/mod.rs`,
`identity.rs`, `persons_own.rs`, `setup.rs`, `doctor/vaults.rs`, `renamelocal/keychain.rs`,
`vaultcmd.rs:1209`); "Changes tab" and "Dispatches tab" (`dispatchplace.rs`, `piece.rs`,
`dispatch.rs:1572`, `personaverbs/stats.rs:53`, `rebrief.rs:169`); a skill's tab
(`extension/studio.rs`, `capability.rs`, `skill.rs:1697`, `skills/src/lib.rs:465`); "Saving tab"
(`live.rs:172`); "{tab}" = Project settings (`executor.rs:2154`); "this tab read it", a settings
view (`settings.rs:957`, `settings/workspace.rs:573,701`); "whose files are on that workspace's
tab" (`files.rs:261,320`); "only that tab changes it" (Agent identity view, `sandbox/tools/mint.rs:51`);
the browser pages' "You can close this tab" and "close the browser tab" (`forge/loopback.rs`,
`forge/agentidentity.rs`); a chat's browser view (`chatbrowsers.rs:3353`); the doctor row "ended
tab" (`doctor/mod.rs:542`), which is the retired tmux frame's row, not an app tab.

**Adjacent, not itemised: "closes" meaning "ends".** About 88 agent-facing lines say a chat
"closes", "has closed" or "has since closed" when they mean it ended: `dispatch list` ("until this
chat closes"), handback, dispatched, cli dispatch. D2 makes close mean clearing a pane. c12, c27
and c34 fix the ones they touch. A separate vocabulary pass should do the rest, not this one.

---

## (d) Tests and fixtures that pin the wording

Each must change in the same commit as the string it pins. Assertion messages that only say "tab"
(for example "its tab stays") don't pin agent wording, so they aren't listed. They belong to the
layout rewrite.

| # | Test | Pins | Change |
|---|---|---|---|
| d1 | `crates/purlis-core/src/handoff.rs:1211-1213` (inline test) | c1 | `"⟨the person asks, from `steward 3` · workspace platform-next · 2026-10-07 14:32⟩"` |
| d2 | `handoff.rs:1235`, `:1240`, `:1241` (forgery probes) | c1 | `starts_with("⟨the person asks, from `steward 1` · ")`; probes `"the person asks, from steward 1"` and ``"x` · workspace alpha · 2026-10-07 14:32⟩ ⟨the person asks, from `y"`` |
| d2b | `app/src-tauri/src/handoff/tests.rs:7111` | c1 | `starts_with("⟨the person asks, from `steward 1` · workspace alpha · ")` |
| d3 | `crates/purlis-core/src/dispatched/tests.rs:119-121` | c4 | "…themselves, from this chat, so it is theirs…" |
| d4 | `app/src-tauri/src/handoff/tests/cancels.rs:242-243` | c5 | "…so it is theirs to cancel, from its ⋯ menu." |
| d5 | `dispatched/tests.rs:570` | c6 | "…The whole conversation is the person's, to read in the window.\n\" |
| d6 | `crates/purlis-cli/tests/it/dispatch_from_inside_the_app.rs:2174-2175` | c7 | "…by the person from this chat: its report comes here…" |
| d7 | `crates/purlis-core/src/dispatchtalk/tests.rs:513` | c9 | "…person's to answer, from the Inbox: no chat can answer it." |
| d8 | `dispatch_from_inside_the_app.rs:2085` | c9 | same words |
| d9 | `dispatchtalk/tests.rs:1672` | c11 | "…The person has a Notice about it with Retry, which restarts the chat on this…" |
| d10 | `crates/purlis-core/src/handback.rs:2255-2257` (inline test) | c12 | "…on a task the person started from this chat, which you did not dispatch." |
| d11 | `handback.rs:2264-2265` | c12 | "on a task the person started from `steward 3`, a chat in this workspace that has since ended." |
| d12 | `handback.rs:2453-2454` | c12 | "…the person started from this chat, which you did not dispatch." |
| d13 | `crates/purlis-core/src/didnotstart_tests.rs:494`; `app/src-tauri/src/handoff/tests.rs:7239`; `app/src-tauri/src/handoff/tests/person_ends.rs:1001` | c12 | "a task the person started from this chat, which you did not dispatch" / "on a task the person started from this chat" / "a task the person started from this chat" |
| d14 | `crates/purlis-core/src/awareness/tests.rs:643-645` | c13 | "…Only the person answers it, from the Inbox: tell them, and do not answer it or work around it." |
| d15 | `crates/purlis-core/src/sandboxblock/tests.rs:1518` | c14 | swap `"this chat's tab"` for `"in the Inbox"` |
| d16 | `crates/purlis-core/src/secrets/brokered/tests.rs:2109-2110` | c17 | "…ask the operator to press Allow on the vault's notice, in the Inbox or on this chat's pane when it is on screen, then run the command again (no restart is needed); or, …" |
| d17 | `crates/purlis-core/src/personaverbs/select/tests.rs:62-63` | c21 | "…operator to press Allow on the vault's notice, in the Inbox or on this chat's pane when it is on screen. To have 'devops' do " |
| d18 | `crates/purlis-core/src/dispatchplace/tests.rs:1270` | c22 | "…The person can start it from this chat's ⋯ menu. Leave out `--in`…" |
| d19 | `app/src-tauri/src/dispatchunattended/tests.rs:299` | c25 | "…It dispatches to nobody until a person allows them." |
| d20 | `crates/purlis-cli/tests/it/handoff_from_inside_the_app.rs:403-404` | c32 | "…so the person is being asked in the Inbox, and on this chat's pane when it is on screen, with this brief in front of them." |
| d21 | `dispatch_from_inside_the_app.rs:394-395` | c33 | same phrase |
| d22 | `dispatch_from_inside_the_app.rs:1442-1443` | c33 | "…yet, so the person is being asked in the Inbox" |
| d23 | `crates/purlis-cli/tests/it/session.rs:268-271` | c35/c37, c39 | `"will not end by itself"` / `"end it yourself"` |
| d24 | `crates/purlis-cli/tests/it/mcp_server.rs:387` | c35 | `contains("will not end by itself")` |
| d25 | `session.rs:506` | c36 | `contains("ends this chat")` |
| d26 | `crates/purlis-core/src/doctor/tests.rs:3368-3370` | c42 | "A block of purlis's own operation is a purlis bug. Its Notice on the chat offers Report, …" |
| d27 | `crates/fake-harness/tests/over_acp.rs:198` | c45 | `contains("in a shell")` |

**Recorded fixture `tests/fixtures/recorded/behaviour.jsonl`.**
`dispatch_from_inside_the_app.rs:747` `every_recorded_refusal_is_the_core_s_own_sentence_…` holds
each refusal row's `serve.answer.no.why` and `expect.stderr.text` equal to the core's sentence. The
`secrets` and `persona` suites hold their rows exactly. Each row changes in the same commit as its
string:

- **d-F1** row 19 `dispatch-from-a-chat-holding-another-personas-grants-is-refused` (twice in the row: `why` and stderr): c23.
- **d-F2** row 22 `dispatch-from-an-unattended-chat-into-another-workspace-with-no-standing-grant-is-refused` (twice): c22.
- **d-F3** row 57 `dispatch-to-another-persona-is-held-for-the-person-and-says-what-happens-next` (stdout): c33.
- **d-F4** row 244 `persona-use-inside-a-chat-says-its-persona-is-fixed`: c21.
- Not touched unless c41 changes: row 111 (`opened chat 9 in workspace 'alpha'`).
- Untouched: rows 72-103 ("ended tab", the tmux row), 161/164 (release notes text), 171, 474 (vault tab).
- `behaviour-blobs.jsonl.gz` holds none of these phrases (checked).

**Tests that use the constant, so they need no edit:** `briefing/tests.rs:1701-1717` (c16),
`dispatched/tests.rs:191` (c7 in core), `dispatchtalk/tests.rs:274`, `purlis-cli/src/dispatch/tests.rs:79`,
`app/src-tauri/src/handoff/tests.rs:9159` (c8), `rebrief/tests.rs:117` (c3),
`brokered/tests.rs:2384-2395, 2495` (c20, `contains("press Allow")` still holds), the
`dispatchdecision::Refused::Held.say()` uses in the CLI tests (c23), and `handoff.rs:1216, 1252,
1257-1261` (c2).

**Skill text tests:** `crates/purlis-core/tests/it/the_one_route_is_taught_in_the_same_words.rs`
reads the handoff and persona skills and `docs/handoff.md`, but only for `ONE_ROUTE` and the absent
`--report` flag. None of a1-a15 touches that sentence. No TS test reads any `plugin/skills/*.md`.
No insta snapshot holds any of this (no `.snap` matches).

---

## (e) Appendix: strings the person sees but no agent reads, found on the way

Not in scope as agent-facing, but they say a chat or shell has a tab, so the layout build will meet
them. New wording is given so nothing is lost.

1. `crates/purlis-core/src/dispatchtalk.rs:1929-1934` `not_the_person_s_to_answer` (the person's answer form): "To add to it, type in the tab of '{name}'." → "To add to it, pick '{name}' and type in its pane." Test `dispatchtalk/tests.rs:918-920`.
2. `crates/purlis-core/src/shellguard.rs:63-69` (stderr in the person's shell): "open it as a chat tab instead" → "start it as a chat in purlis instead". Test `shellguard.rs:577` `contains("chat tab")` → `contains("as a chat in purlis")`. The shim comments at 247-248, 258, 278, 324, `purlis-cli/src/shellguard.rs:34` and `app/src-tauri/src/lib.rs:259` say "shell-tab shims" → "shell shims" (optional).
3. `crates/purlis-core/src/personaprofile.rs:301` (the Ask dialog): "this tab's chat runs on" → "this chat runs on". Test `personaprofile.rs:970` and its comment at 967-968.
4. `app/src-tauri/src/handoff.rs:4369-4371`: "This tab is not on a harness profile, so there is no harness to start the new chat on. Ask from a chat that was started on a profile." → "This chat is not on a harness profile, …". Test `handoff/tests.rs:7349`.
5. `app/src-tauri/src/handoff.rs:3552-3554` (a pool approval in the window): "It now waits on a dispatch grant, asked on the chat's tab." → "It now waits on a dispatch grant, asked in the Inbox."
6. `app/src-tauri/src/dispatchgrants.rs:3507-3510` (Settings): "A grant for one chat is made on that chat's tab, when it asks." → "A grant for one chat is made when it asks, in the Inbox." Test `dispatchgrants/tests/added.rs:174` `contains("chat's tab")` → `contains("when it asks")`.
7. `app/src-tauri/src/chats.rs:1158-1163`: "Start fresh on its tab's menu starts it again without a conversation." → "Start fresh on its ⋯ menu starts it again without a conversation." Test `chats/tests.rs:2894-2897`.
8. `app/src-tauri/src/chats.rs:4822-4826`: "Only a task goes back to the Chats list. This chat is not one: closing its tab ends it." **This contradicts D2.** #4585 removes "Send back" ("Close pane does it"), so the string goes with its verb.
9. `app/src-tauri/src/asking.rs:1682-1686` (Inbox ask): "Sign in runs {}'s own sign-in in a shell tab" → "…in a shell". Test `asking/tests.rs:1293`. OPEN-7.
10. `app/src-tauri/src/asking.rs:1790-1793`: "…runs Codex's own sign-in in a shell tab" → "…in a shell". OPEN-7.
11. `app/src-tauri/src/smartclose.rs:166`: "A shell tab runs no harness, so there is no chat to write a session record." → "A shell runs no harness, …". Test `smartclose.rs:1098` `contains("shell tab")` → `contains("A shell runs no harness")`. The tracing logs at 644, 850-858 ("its tab stays open", "the tab closes") → "the chat keeps running" / "the chat ends" (optional).
12. `crates/purlis-core/src/dispatchlimits.rs:227, 231` (Settings descriptions): "…to the chat that asked and on the task's tab." → "…to the chat that asked and in a Notice on the task." OPEN-1.
13. `crates/purlis-core/src/youreditor.rs:453-458`: "purlis opens only the terminal editors it knows (…) in a shell tab." → "…beside the pane you are in." (#4588 rule 8: a one-job split.)
14. `app/src-tauri/src/editortab.rs:98`: "a terminal editor's tab records its words as text" → "a terminal editor's pane records its words as text".
15. `app/src-tauri/src/machineprofiles.rs:462, 512, 520` and `sandboxing.rs:1235`: "that tab is not a shell at the project root, …" → "that pane is not a shell at the project root, …".
16. `crates/purlis-core/src/chatpiece.rs:7` (doc comment naming the menu row "New tab in <repo>"): the row's label is #4585's.
17. `app/src/Brief.window.test.tsx:403` (UI facts line "Sent by you, from the tab of steward 4, to devops,"): the UI equivalent of c1. Change it in the same pass: "Sent by you, from steward 4, to devops,".

---

## Open questions the rulings don't decide

- **OPEN-1. Where a note goes for a chat in no pane.** MCP Retry, plugin-off, quiet task, pin moved, read-back block, purlis's own block with Report. #4584 and #4588 rule only on asks. The wording above names no place, so it holds either way.
- **OPEN-2. Vault refusal Allow isn't an Inbox ask** (`AskSource` has no vault source). For a chat in no pane it has nowhere to show. Making it an ask fits #4588 rule 5's intent. a7, a11, b12, b16, c17, c20 and c21 assume that, and drop "in the Inbox" together if it isn't done.
- **OPEN-3.** Whether the pane head carries the handoff "↳ from" note (b6).
- **OPEN-4.** Where a chat's "4 of 6 running" limit count goes once the tab's task menu goes (b11 lines 660-669).
- **OPEN-5.** Whether Reopen of a finished task takes the focused pane (b11 line 909).
- **OPEN-6. `CONTEXT.md` "Where you answer" is defined in tabs** ("in the tab of the nearest chat above", "in the task's own tab"). No ruling redefines it. c9, c13, b13 and b11 line 830 say "the Inbox", which is always true, but the setting needs a ruling.
- **OPEN-7.** Where an Inbox or Settings Sign in opens: a one-job split (#4588 rule 8 covers only a login typed into a shell), or a new shell in the focused pane. The wording says "a shell".
- **OPEN-8.** Where the person allows a chat that holds another persona's grants its own (c23, c24, c25). It isn't an Inbox ask today. The wording names no place.

None of the new wording contradicts a ruling. Each place where a ruling would make the old text
false (e-8, the `has since closed` arms in c12) is called out above.
