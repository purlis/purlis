# A chat's input keeps its order in a lane of its own, and never waits on the host on the window's thread

**Accepted 2026-11-02**, for #4734 of spec #4724 (felt speed under load). It replaces the rule the
window's terminal commands kept since #891 and #1949, written in `off_the_main_thread.rs` and in
the `INPUT_ORDER` reason of the command-cost contract (`off_the_main_thread/tests/costs.rs`):
*"A terminal's own commands stay synchronous on purpose … the window relies on their order: a
pane's resize must land before the watch that follows it (#891)."*

## Where purlis was

Tauri keeps the order of a window's asks only for synchronous commands, which run on the main
thread: the one that draws the window. So a pane's keys (`send_input`, `send_input_bytes`), its
resizes (`resize_session`) and its view's open and close (`watch_session`, `unwatch_session`) were
synchronous, and each went to the session host and waited for its answer there. The rule assumed
that took well under 5 ms. Since chats run in the session host (#1998), the wait is a round trip
over a link: `HostLink::call` waits up to 30 s for an answer, and a link still being made waits up
to 30 s more on a condition variable. A host that was slow, busy or coming up froze the whole
window, every project in it, for as long as that took.

## The decision

- **Each chat has a lane** (`inputlanes::Lanes`, one set for each project held). A pane's keys,
  paste, mouse report, resize and view close join the back of their chat's lane and the command
  answers at once, on the window's thread, having waited on nothing. A worker of the lane's own
  sends each to the host in turn, and is gone once the lane is empty. A chat's lane never waits on
  another chat's.
- **The order is the order of the asks.** The commands stay synchronous, but for one reason only:
  Tauri hands them over in the order the window asked, so they join their lane in that order. A
  paste, the key after it and the resize after that reach the host in that order, however long
  the host takes over each.
- **A view's open answers off the window's thread.** `watch_session` must answer with the view's
  id and size, so it is `async`: it takes its turn in the chat's lane on a blocking thread and
  answers once the host has. It still lands after every resize the pane sent before it, which is
  what #891 asked for. The pane already waits for that answer before anything that depends on the
  view.
- **Everything else that types into a chat takes its turn in the same lane, and waits for it**
  (`Lanes::in_turn`): a smart close's prompt, a stop's line, a reference, a curation prompt, a
  vault's or a browser's word. It runs once what was sent before it has been, and nothing sent
  after it runs until it has, so a prompt never lands in the middle of a paste.
- **What types into a chat waits for its lane for 1 s at most** (`inputlanes::A_TURN_WAITS_AT_MOST`,
  `Lanes::in_turn_within`). Most of it is asked on the window's thread by commands that stay
  synchronous for their order (`INPUT_ORDER`: a smart close's prompt before its cancel, a stop,
  a close before the resume that takes its place). Without a bound, Stop on a chat whose lane is
  stuck behind a host that does not answer would freeze the window for that lane's wait and
  then its own: up to about 60 s, where it was 30 s before. A lane that has not emptied in 1 s is
  stuck, so the command is refused and types nothing. A stop then ends the chat as it stands, as
  it does when there is nothing to write to. Its own call to the host still waits as it did
  before this decision. Making these commands `async` was weighed and not chosen: Tauri keeps
  no order between `async` commands, and these depend on theirs. A view's open is already off
  the window's thread and waits for its turn without a bound.
- **What the person's keys change in memory is changed as they are asked**, on the thread that
  asked, before the keys join the lane: a curation prompt still waiting is dropped, a smart close
  is cancelled, a typed `/purlis-smart-close` is marked. A worker only writes to the host, takes no
  lock of the app's and so is never held by a caller in its turn.
- **A send that fails is said in the chat's pane, and the keys behind it are not sent.** The
  window is told (`chat-input-refused`) and draws a Notice on the pane with the reason and
  Dismiss. Half of what the person typed is worse than none, so the keys, pastes and mouse
  reports already queued behind the one that failed are dropped, unsaid; the next one after the
  lane has emptied tries again. **A resize and a view's close are never dropped**
  (`Lanes::send_always`): a view left open is one the host goes on feeding, and paces, for the
  rest of the session. A resize or a view's close the host refuses is logged, not said: the
  pane's next one says it again.
- **A chat whose link is down fails fast.** A chat whose runner is reconnecting (#2496) refuses
  its input and resizes at once, without asking the host, so the lane never fills behind a link
  that is not there.

## Consequences

- `INPUT_ORDER` in the command-cost contract no longer lists the pane's terminal commands. They
  are listed under `QUEUED` with this reason, and `watch_session` is off the list, as `async`.
  The contract's tests hold the change: no input, resize or view waits on a slow host on the
  window's thread, and a paste, keys and resizes reach the host in the order the pane sent them.
- A refusal the pane used to write into the terminal in red, as the answer to its ask, now comes
  later, as a Notice. The ask itself answers with a refusal only where the project is not held.
  The Inbox's Send reply goes the same way (`send_input`): its reply box is cleared once the
  reply is queued, and a host's refusal of it is said on the chat's pane, not under the box.
- A host that does not answer still holds a chat's lane for up to 30 s, off the window's thread:
  that chat's keys wait, and every other chat and the window go on. The fast failure covers a
  chat whose runner is reconnecting; a project link still being made is waited for by the
  worker, not the window.
- A lane's worker is a thread started when the lane is first sent to and gone when it empties.
  A host answers a key in about 1 ms and a person types one about every 100 ms, so the lane
  empties between keys: typing starts one thread per key, and a burst (a paste, keys sent
  faster than the host answers) shares one. A thread's start costs tens of microseconds, well
  under a frame, so the worker is not kept waiting for more.
- A turn taken on a free lane hands the lane on however it ends, a panic included, so a turn
  that fails never leaves its chat's keys waiting for good.

## Amendments

The amendments to this decision are files of their own in [`0108/`](0108/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record, so two pull requests that amend it do not touch the same lines
(#1982).
