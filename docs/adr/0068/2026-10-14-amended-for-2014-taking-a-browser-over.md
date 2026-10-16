# Amended for #2014, taking a browser over, 2026-10-14

The person takes a browser over from its view and drives it themselves; while they drive, the
agent's browser tools are refused in words, and the browser returns to the agent only when the
person hands it back. That is the operator's ruling of 2026-10-14 (spec #2008's grill, "agree at
all"): a lease the person took parks with nobody until Done; clicking or typing into a browser
view takes it over at once, and Done or one fixed chord hands it back; the agent never sees what
the person typed. The amendments above are left as accepted, and this note adds to them. Items
1 to 7 are **decided in implementation, to be confirmed** (D-2014-1 to D-2014-11).

1. **Session protocol 1.12, a minor.** `take_over` and `hand_back`, each answered `driving` with
   who drives now; `browser_input`, one event of the person's (a mouse's or a key's) aimed at
   what a frame showed, refused `not_driving` unless the person drives and `input_missed` where
   the page has moved on; `size_browser`, the person's page size, refused `not_driving` too; and a
   listed browser's `driver`. All four are `local-ui`'s alone, with a row of their own in FD-27's
   table for every scope, and keep their order in the browser's program lane, so none waits
   behind an act in flight. Recorded in the protocol's fixture. *D-2014-1, to be confirmed.*
2. **The lease is the session host's.** The host holds each browser's driver and the epoch of its
   lease; no link's end changes it, so a crash of the window leaves the browser with the person,
   and a view opened again says so. The engine keeps the same epoch and checks it where an act
   reaches a page: an act is carried out under the epoch the host checked, and once a take-over
   returns, no act of the agent's reaches a page. *D-2014-2, to be confirmed.*
3. **While the person drives, the agent reads nothing and acts on nothing.** Every call is
   refused in words, reads as well as actions. A call in flight answers "interrupted, state
   unknown", whatever it had read. The hand-back voids every reference dealt before it, so the
   agent's next act starts from a new snapshot. The agent is told nothing of what the person did,
   and nothing they did is replayed. *D-2014-3, D-2014-10, to be confirmed.*
4. **What the person types or pastes stays out of every answer the agent gets.** Before each key
   that may change what a field holds (one that types, a deletion, a chord such as a paste), the
   element that has the focus is blanked, so it is in no later snapshot, find, wait or screenshot.
   After the key, what the field holds is read from the field itself, and again at a press of the
   mouse and at the hand-back; with each run of characters as the person typed it, it is kept out
   of every later answer about every page of the browser, a page opened later included, whatever
   became of the field. A page that cannot say which element has the focus, or has one focused
   that its tree does not show, has the key held back, and the person is told. A character typed
   with Alt or AltGr (Option on a Mac) types, blanked first like any other. *D-2014-4, to be
   confirmed.*
5. **Input is aimed.** A browser view's frames name what they show, the page and its documents
   (a number that changes when the page or any frame inside it loads another document; a frame
   format of its own, sent to 1.12 links), and each event of the person's names the frame it was
   aimed at. The engine drops an event whose page has moved on in any of its frames, so input
   aimed at one frame's document never lands on another's; nothing is replayed. *D-2014-5,
   decided by the dispatcher.*
6. **Taking over and handing back, in the window.** The first click or key in a browser view takes
   the browser over and reaches the page; the bar purlis draws above the page says the agent is
   paused. Done, or ⌘⇧↩ (Ctrl+Shift+Enter off a Mac), hands it back; nothing else does. While the
   person drives, a Control chord stays with the page and the palette's pass-through key, pressed
   again, reaches it, as over a terminal. Size presets (laptop, desktop, tablet, phone) are the
   person's, so picking one takes the browser over. *D-2014-6 to D-2014-9, to be confirmed.*
7. **The browser's reach is unchanged.** The person's input reaches the page through the same
   engine and the same reach proxy; taking over adds no way to a site the chat could not reach.
   *D-2014-11, to be confirmed.*
