# A done todo is kept, and can be reopened

**Accepted 2026-10-26**, by the operator's ruling of that day on #3063: closing a todo marks it
done instead of deleting it. It amends ADR 0004 of the Python charter
([`purlis/purlis-plane`](https://github.com/purlis/purlis-plane), *"Intent is its own store, not a
state flag on memory"*), and the rule its successors wrote down from it: *"there is no state
field: a closed todo is a deleted file whose trace is the journal's"* (ADR 0006, the plane
format). It carries R131 of spec #2897: in the Todos view a checkbox toggles done, which strikes
the text through.

## Where purlis was

A todo was open or gone. `ws todo done <slug>`, the MCP tool `todo_done` and the window's Mark done
wrote `Closed todo: <title>` into the workspace's journal and then deleted the todo's file and its
index line. Nothing could be reopened: the only trace was a journal entry, and putting the todo
back meant re-recording it by hand under a new stamp, which hid how long it had waited.

ADR 0004's reason still holds: intent expires, and a done todo must not read as outstanding work
or as a fact. What it did not need was for the file to go.

## The decision

- **A done todo moves to `todos/archive/`.** Closing one still writes the journal's entry first,
  then moves the file, unchanged and under its own name, out of `todos/` and drops its index line.
  The move is the store's one retire, the one the journal's Delete uses
  (`memstore::archive_one`): exact names only, link-gated, held by descriptor, and numbered `-2`
  when a done todo already has the name.
- **Still no state field.** Which folder a todo's file is in says whether it is open or done. Every
  reader that asks "what is left to do" reads `todos/` and nothing below it, so the session-start
  digest, the status line's count, `workspace remove`'s warning, duplicate detection, the sidebar
  and `todo_list` count and list the open ones only, as before.
- **Reopen brings it back as it was**: the file moves back to `todos/` under its name and its index
  line is appended (`memstore::unarchive`). Its stamp is the one it was opened with. The journal
  keeps the close it recorded; a reopen adds nothing to it.
- **Delete still removes one, open or done**, and deleting a done one is the person's act.
  `ws todo forget <slug>` at a terminal and the window's Delete drop the file and journal nothing.
  A chat's `forget` (the app writes it for the chat, ADR 0067 §2) reaches the open todos only, as
  it always did, and is refused for a done one with the way to delete it yourself. Reopen and
  Delete take a done todo's exact name; a slug typed on a command line is made exact first.
- **A promoted todo is still deleted** (ADR 0088 §5): the issue it went to is where it is tracked.

## Where it is said

- `purlis ws todo done <slug>` keeps the todo and says how to reopen it; `purlis ws todo reopen
  <slug>` reopens one; `purlis ws todo --done` lists the done ones; `forget` deletes either.
- The MCP tool `todo_done` keeps the todo too, and says how to reopen it. `todo_list` lists the open
  ones.
- The Todos view draws a done todo after the open ones, struck through with its box ticked;
  unticking reopens it, and its menu has Reopen and Delete.

## Consequences

- `todos/archive/` is committed with a LIVE workspace, as `todos/**` already was, so a done todo
  reaches the operator's other devices. The window watches it, so the Done tab follows a change
  made elsewhere.
- `workspace fork` copies `todos/` whole, so a fork carries the parent's done todos as well as its
  open ones: what was already done is part of the context a fork is for, and the two lists go
  their own ways from there.
- A chat's MCP tools cannot list or reopen a done todo yet: `todo_list` lists the open ones and
  there is no `todo_reopen`. A chat reopens one with `purlis ws todo reopen`; the tools are #3416.
- A workspace's done todos grow until someone deletes them. They are small files, and nothing reads
  them on a hot path; a bound is a later decision if one is ever needed.
- The index header a new todo store is written with still says *"done removes it"*. It is about the
  index, whose lines are the open todos, and a done todo's line does go; it is left as it was so a
  store written before and after this reads the same.
