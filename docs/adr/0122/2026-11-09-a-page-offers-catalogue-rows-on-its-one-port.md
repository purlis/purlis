# A page offers catalogue rows on its one port, and purlis's bar draws them

**2026-11-09, #5972.** Amends §8 of ADR 0122 with what building offered actions decided.

- **An offer names a catalogue row; it never words one.** The page posts the whole set it
  offers now, `{ id, offers: [{ does, subject }] }`, on `purlis.port`, carried as
  `/.purlis/port/<load>/offers/<id>/<n>?<{offers}>`, `<n>` counting the load's sets from 1, so
  the set said last is the one drawn however the sets are gated (#6232). `does` is the part of an `actions.ts` row id
  before its colon and `subject` the part after it, so `todo.done` with `fix-login` names the row
  `todo.done:fix-login`. Each set replaces the last; an empty one withdraws them all, and so
  does the page loading again. At most 8, each in closed types: a field more (a title of the
  page's own, say) refuses the whole set (`crates/purlis-core/src/artifact/offers.rs`).
- **It is gated as a read is.** Only the load whose port is open, and only while the pin and
  this machine's approval still run the page's script: a page whose script has not been
  approved offers nothing (`app/src-tauri/src/artifacts/offers.rs`). The page hears how many
  were taken, or `not-approved` or `refused`.
- **The window draws only the rows it built.** A closed list in `app/src/pageOffers.ts` names
  what a page may offer: the rows about the things a page can read through its queries, today
  the todos (`todo.open`, `todo.done`, `todo.reopen`, `todo.forget`, `todo.delete`). The row is
  looked up in the catalogue the window built, and drawn only when it is about the artifact's
  own workspace, in the catalogue's words. Anything else is refused with a sentence on the bar
  and never drawn. A press runs the row as the palette does.
- **A delete always asks.** `todo.forget` and `todo.delete` open a question naming the artifact
  that offered it; only its answer runs the row. Cancel and Esc do nothing.
- **No new command, and nothing a chat can reach.** An offer is an event to the page's own
  window; what it names is done only by that window's own action path.
