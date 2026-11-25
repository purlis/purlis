# Amended (2026-11-25, M74.1 T1.1-68 part C, #6957, #6646; Q304 Q3): a chat proposes a memory or todo for a store that is not its own, and only the person writes it

A sandboxed chat writes only the memory and todos of the persona it runs as, of shared memory and
of its own workspace. A write aimed at another persona or another workspace was refused, with no
one asked and "ask the person to do it from the window" as the way out, which named a control that
did not exist (R-328). Q304 Q3 ruled that it becomes a proposed change, shown with Apply and
Dismiss and its text.

- **Which verbs.** `purlis persona remember <other> "<text>"`, `purlis workspace remember` and
  `note` with `-w <other>`, and `purlis workspace todo -w <other> "<text>"` add one entry and
  remove or rewrite nothing. By the person's ruling of 2026-11-25, `purlis persona forget <other>
  <slug>`, `purlis persona edit-memory <other> <slug>`, `purlis workspace forget -w <other>
  <slug>` and `purlis workspace edit -w <other> <slug>` are proposed too, each shown as the exact
  diff: a forget shows the whole entry being removed, and an edit shows the entry before and
  after, built by the same code that writes it (`memstore::edited`), so the text on the row is the
  text written. `move-memory`, `optimize --apply`, `archive`, `unarchive`, a todo's close,
  `vision`, shared memory named through another persona and `--ephemeral` scratch stay refused,
  with their old words, and the app is not asked. The app refuses any other write too
  (`purlis_core::memoryproposal::shown`), whatever a line carries.
- **The ask.** `Ask::ProposeMemory` carries a write and a target (a persona or a workspace). It
  names no chat's own store and never who asks: the chat is the one whose token the line carries.
  This is the one write that names a place, and it is held, not made: the target must be a persona
  that loads or a workspace that is there, never `_shared` (reached with `--shared`), and the
  write must fit it (a persona's memory for a persona, a workspace's memory or todo for a
  workspace).
- **Held beside the other proposals.** The proposals registry holds it as a `MemoryChange`, as it
  holds a limit, skills and a pin: shown in the Inbox and on the chat's tab with its target and the
  whole text, under the same rate, the same cap of eight a chat and the same policy `off`
  (`proposed-changes`, PC-14). Several to one store are held side by side, since each adds an
  entry. A text that draws as something other than it is (a bidi override) is not held.
- **Apply writes exactly what was shown, or nothing.** It judges the write again, records the
  intent before it writes and the outcome after, then makes it with `brokered::answer` (the one
  `brokered::perform` calls, which a forget and an edit need, since they answer in lines), with
  the target standing where the chat's own store would. A forget or an edit is pinned to the
  entry's text as it was shown, and acts on the entry by its exact name: an entry that changed
  since is **stale**, nothing is written, Dismiss is all the row offers, and the chat is told to
  read it again and propose again. A store or entry gone since is "unprepared": nothing is
  written. What only adds is never stale. The chat is told on its next turn that it was applied or
  dismissed.
- **No app, no change.** Where no app takes the ask (no chat, no connection, an app older than the
  ask), the command's refusal stands in its own words. Where the app holds nothing (the cap, policy
  off, a store that is not there), the chat is told why.

Nothing here adds a store: the proposals registry is in memory, and what Apply writes goes to the
tiers `docs/plane-format.md` already names for a persona's and a workspace's memory and todos.
