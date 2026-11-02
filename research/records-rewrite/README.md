# Rewriting the records for chats, tabs and panes (#4704)

The answer to [#4704](https://github.com/purlis/purlis/issues/4704) on the map
[#4575](https://github.com/purlis/purlis/issues/4575). It lists every record that still says a chat
is a tab, and what each one becomes. **It's a list, not the edits.** It goes into the spec.

Read at origin/main `421410a1b` on 2026-11-02. Nothing was edited, built or run. This is a
throwaway branch and will never be merged.

| File | What it holds |
|---|---|
| [`adrs.md`](adrs.md) | every ADR to amend: the stale lines, the ruling that overturns them, and the amendment's gist |
| [`glossary-and-docs.md`](glossary-and-docs.md) | `CONTEXT.md` entries in full, `docs/`, `crates/purlis-core/docs/handoff.md`, the site and README |
| [`agent-facing.md`](agent-facing.md) | skills, the docs `purlis docs show` serves, Rust and CLI strings agents read, the tests that pin them, and strings only the person sees |
| [`rulings.md`](rulings.md) | the map's rulings the three files cite, copied from the issues |

**Where a file marks ⚠, ❓ or OPEN, the Decisions section below settles it or sends it to the spec.**

## Counts by kind

| Kind | Count |
|---|---|
| ADR amendments, each with its gist | 23 ADRs, plus about 40 wording-only amendments ("on the chat's tab" → "on its pane"), which one sweep fixes ([`adrs.md`](adrs.md) §B) |
| ADR titles that are now wrong | 2: 0062 ("shell tab") and 0064 ("closes the tab"). Both get an amendment, not a superseding ADR, because each one's decision still stands |
| Glossary (`CONTEXT.md`) | 34 entries: 5 new (Tab, Pane, Empty pane, One-job pane, Tab window), 8 rewritten in full (Chat, Task, Handoff, Where you answer, Strip, Pin, Shell, Smart close), 1 retired (Headless chat), the rest word swaps |
| Docs | 79 items in `docs/` (5 full rewrites, among them `spec.md`'s terminal-editor line and `plane-format.md`'s `reopen.json` `tabs[]`), 40 in `handoff.md` (its "how you see a handoff" section, :91-123, rewritten in full), 2 on the site and README |
| Skills | 14 passages in 5 shipped skills: handoff 6, secrets 4, smart-close 2, persona 1, safe-remove 1. Theme has 1 optional |
| Agent-facing strings | 45 in Rust and the CLI: the person-asks stamp in `handoff.rs`, refusals, sandbox, vault and host strings, two MCP tool descriptions (`session_record` "says whether the tab now closes"), CLI help, and six smart-close answers. Plus 20 passages in docs `purlis docs show` serves (these overlap the handoff.md items) |
| Tests that pin the old wording | 28 rows in 20 files, plus 4 rows of `tests/fixtures/recorded/behaviour.jsonl` (19, 22, 57, 244) |
| Strings only the person sees | 17, found along the way ([`agent-facing.md`](agent-facing.md) §e). `chats.rs` "closing its tab ends it" contradicts D2 |
| Code doc comments | about 6 (`reopen.rs` `pinned`, `tab_opened`, `Mode::Handoff` and `Mode::Task`, and `tabs.ts` `renameTab`). They change with the code #4587 and #4700 rewrite |

## What must change together

**The records follow the behaviour, not ahead of it.** Agent-facing text is read as instructions.
A skill that tells an agent "the chat takes no pane" before the build ships would be as wrong as
the old text is now. So each group below lands in the PR that ships its behaviour.

1. **Handoff and dispatch** (ships with the sidebar-only start, D4 and #4588 rules 5 and 6):
   - ADR 0090, and 0073 and 0080 V24a;
   - `CONTEXT.md`'s Chat, Task, Handoff and Needs you entries, with Headless chat retired;
   - the handoff and persona skills, and `handoff.md` :91-123 and its line items;
   - the person-asks stamp in `handoff.rs`, its tests and the fixture rows;
   - the dispatch refusal strings and their pinning tests.
2. **Where an ask is answered**, one phrase: "in the Inbox, and on this chat's pane when it is on
   screen". It appears in 16 places across the handoff, persona, secrets and safe-remove skills,
   `handoff.md` and the Rust strings.
   - Recommended: one constant, held word for word by a test, like `handoff::ONE_ROUTE`
     ([`agent-facing.md`](agent-facing.md), "Words used throughout").
3. **Every Rust string with its pinning test and its `behaviour.jsonl` row**, in one commit.
   - A test checks the fixture against the core's sentences, so the two can't land apart.
4. **Pins** (with #4700's code):
   - ADR 0039 (the 2026-09-22 sentence goes; the 2026-09-28 chip amendment retires);
   - ADR 0040, whose reason flips: the pin now outlives its contents;
   - `CONTEXT.md` Pin;
   - `plane-format.md`'s `reopen.json` (`chats[].pinned` was never documented);
   - the doc comments in `reopen.rs` and `tabs.ts`.
5. **Smart close** (with #4585 rule 5):
   - ADR 0064;
   - the smart-close skill;
   - `session.rs`'s answers;
   - the `session_record` tool description;
   - `CONTEXT.md` Smart close;
   - `spec.md` SI-8f.
6. **The tab tree and windows** (with #4587 and #4701):
   - ADRs 0036, 0033 and 0034;
   - ADR 0076, where "open" stops meaning "has a tab" and `closed` comes only from End, Smart close or Start fresh;
   - `CONTEXT.md` Tab, Pane, Empty pane and Tab window;
   - `plane-format.md`'s `tabs[]`.
7. **Keys** (with #4699): `ui-primitives.md`'s task-keys table, xterm note and Focus mode line, and
   the headers of `taskKeys.ts` and `sideKeys.ts`.
8. **Views** (with #4703, #4705 and #4702): the ADR 0043 amendment, the narrow-window folding, and `CONTEXT.md`'s view entries.
9. **Sandbox and chat marks** (with #4585's pane head): ADRs 0067 SD-3 and 0096, and `CONTEXT.md`'s Sandbox level and Plane updated.
10. **Chat actions on the chat** (with #4585): ADRs 0084 (Review is a row of the chat's list),
    0102 (Use skill… and the skills chip), 0081 (`reveal`) and 0061, plus `ui-primitives.md`'s pane rows.

## Decisions taken while resolving

- **A level-1 dispatch (the person, 2026-11-02, ruled A).** D4 holds: the dispatch starts in no
  pane, and it's announced like a handoff (chip card, Inbox Update, the row's "new" mark).
  - Its row keeps the faint "can't report asks" hand (`spec.md:70-72`).
  - The asking chat is told the person must look at it.
  - ADR 0090's guard ("opens it as a tab instead, and says so") retires, and so does `CONTEXT.md`'s
    Headless chat with its "Never a level-1 chat". "Headless chat" goes into Chat's _Avoid_ list.
- **Decided without asking** (minor; none crosses a ruling):
  - **V24a's start level** (ADRs 0073 and 0080) is keyed on who started the chat, not on a tab.
    A chat the person starts, and a handoff, start in their terminal. A task starts at the highest
    level its harness offers. That's today's behaviour, so nothing changes silently.
  - **ADR 0067 SD-3's badge** goes on every pane head that shows a chat, and on the chat's sidebar
    row in every scope that lists it. "Cannot be hidden" now means that.
  - **ADR 0095's Sign in** is a one-job split beside the focused pane (#4588 rule 8 names a forge
    login typed into a shell), and it ends when its program ends.
    - Pressed from Settings, the Inbox or the welcome, it opens beside the focused pane, which
      always exists (#4588 rule 1).
  - **ADR 0081's `reveal` from an editor** counts as the person's press (D1 and #4588 rules 1 to 3),
    with focus on the pane head.
  - **The tab's task chip goes.** A chat's tasks are under its row and on ⌘⇧J (#4699).
  - **ADR 0036's amendment** cites #4583's drag exception: "never another workspace's chat" now
    holds for presses only.
  - **#4706 (closed):** ADR 0033's tab-window amendment adds one line. Tabs and panes move between
    windows only by the Move to window rows and tear-off. Zones and placement mode work inside one
    window.

## Left to the spec, with a recommendation

These are gaps, not contradictions: no ruling decides them.

1. **A plain note (not an ask) from a chat in no pane.** Put it in the Notices centre and mark the
   row. Add "Notices centre" to the glossary.
2. **A vault refusal isn't an Inbox ask today.** Make it one, so the seven strings that say "in the
   Inbox" hold. Otherwise those strings drop "in the Inbox" together.
3. **`CONTEXT.md`'s "Where you answer"** is still defined in tabs. New meaning: the asking chat's
   pane when it's on screen, else the Inbox.
4. **The strip's `+`.** It stays "New tab" and opens a tab with an empty pane. New chat in the
   sidebar and the pane takes the focused pane, as D4 says.
5. **Smart close's place in End…'s dialog.** ADR 0064 calls it "primary", and #4585 calls it the
   third answer. Recommend that End keeps the keyboard and the amendment drops "primary".
6. **Closing a browser view's pane.** It ends the browser as today: D2 rules on chats, not views.
   It needs one line in the 0043 or 0068 amendment.
7. **Smaller, in the files:**
   - show-more's needs-you count;
   - the pane head's slot for the plane-updated and sandbox marks;
   - the "Tab or pane switch ≤ 100 ms" budget, which #4692 measures;
   - the theme manifest's `background` variant and its "Session tabs" heading;
   - whether Resume is refused after the kill switch until it's re-armed;
   - whether Reopen of a finished task takes the focused pane. Recommend yes, it's a press;
   - `spec.md:75`'s "free split panes", which contradicts D6 and becomes "a tree of split panes".
