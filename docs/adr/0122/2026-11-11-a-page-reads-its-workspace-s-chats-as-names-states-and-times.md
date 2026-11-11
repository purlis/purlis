# A page reads its workspace's chats as names, states and times

**2026-11-11, #6235.** Amends §7 of ADR 0122 with a seventh core query, `purlis.chats`, and
records why it is safe for a page. These are the operator's rulings of 2026-11-11: *"Live
chats, this workspace, with an option to get historicals too; by default only live."* Of the
three questions the work raised, the operator ruled 1a, 2a and 3a, recorded below.

- **What it answers.** The chats of the page's own workspace, as the window's Chats tree has
  them. Each chat has its id, name, persona, harness, its Chat state (ADR 0121 decision 17) and
  the fact that decided it (`cause`), when it started and last moved, the chat it came from and
  how (`dispatched` or `handed-off-from`), and its newest session record's file name, which
  `purlis.session_record_read` takes. The lineage is the app's own record (`OpenChat.from`),
  the same one the sidebar's tree is drawn from. Nothing is read from a harness's output
  (ADR 0076).
- **Live by default. `history` widens it**, and is declared under `widens`, so a page's
  approval names it as it names `todo_list`'s `done`. With `history: true`, the answer also
  holds the workspace's ended dispatches (tasks and handoffs), newest first: the last 50, or
  `limit`, at most 200, each followed by the chat that asked for it. **History is the dispatch
  records only** (ruling 2a), with no new store: a closed chat that dispatched nothing appears
  in no record and is not listed, and the parameter says so.
- **The state is the core's eight Chat states, from facts the app's Rust holds** (ruling 3a):
  the board's state, the needs-you queue, the stuck watch, a task's report and outcome, its
  tasks at work, its own work and work left behind. They are read in `shownState.ts`'s order
  (`catalogue::chats::mark`), and `shownState.ts` is not ported. The window keeps its finer
  words. A page gets the mark the window's badge wears and the window's name for it
  (`state_said`, "Waiting on you"), not the row's word ("asking the steward").
- **Why it is safe for a page.**
  - *Workspace-scoped*: a page reads its own workspace's chats only, and only as declared. The
    chat tool reads the asking chat's workspace from the app's record of that chat, never from
    the call.
  - *Metadata only*: names, personas, harnesses, states, causes, times and links. Never a
    transcript, a prompt, a brief, a report, a message between chats, or anything from a vault.
    History leaves out each record's brief and report too. A chat's name is what its tab
    already shows anyone at the window.
  - *Read-only*: nothing is made, moved or written by an answer. *When a chat last moved* is
    the app's in-memory note of its board's moves, never written. So `purlis.chats` adds no
    store, and `docs/plane-format.md` is unchanged.
- **Answered by the app, which alone holds the live chats.** `catalogue::answer::ask` reads the
  plane and has no chats, so it answers `failed` for this query. A page's call and its
  subscription are admitted as any query is, then answered by the app (`chatlisting`). A
  subscription is asked again on each move of its project's board, at most once a second as
  every feed is (#5970); no plane change moves it.
- **Its MCP twin, `chats`, is not pre-allowed** (ruling 1a). The catalogue holds that every
  query is a read-only MCP tool, so any chat in a workspace can now list its workspace's
  chats, more than `persona_where`'s same-persona view. It is answered by the app over the hook
  socket (`hookwire::Ask::Chats`), as `persona_where` is. It is not in
  `chattools::PRE_ALLOWED`, so the harness asks the person before a chat lists the others.
- **Not yet.** A helper (a harness's own sub-agent) is no chat in purlis's record, so no chat
  has `helper` as its relation. A chat the person opened has a session record in its answer
  only once a dispatch record names it.
