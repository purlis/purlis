# What switching a pane's chat costs

Research for #4579, under the map #4575 and the decisions in #4576. Read at `origin/main`
7e723d5d4, 2026-11-02. File references are to that commit.

**Nothing here was measured for this ticket.** `uptime` read 43.9 at the start and 32.2 near the
end, over the limit of 20 the brief set, so no bench ran. Every number below is either an older
record, cited with its source, or arithmetic, labelled as such.

## The answer

- **Switching a pane's chat rebuilds its terminal.** The old xterm is disposed and its view closed.
  A new xterm opens a new view, and the session host sends the whole screen and scrollback again.
- **The rebuild's cost follows the chat being shown, not the number of running chats.** A chat with
  no pane has nothing in the window, so 20 or 200 running chats add no terminal work to a switch.
  They can only add React work in `PlaneView`, which nobody has counted for this path.
- **The only recorded time predates the session host.** The worst case is 48 ms against the 100 ms
  budget (L2), from the M0 skeleton. It has not been re-measured since chats moved to `purlisd`.
- **ADR 0086's budgets don't support a number of kept terminals.** M2's only reading is already at
  its ceiling, and no bench measures what a pane costs in the web view.
- **So the code doesn't show that local switching needs a terminal pool.** That is for the
  prototype's measurements to settle. A pool looks necessary for chats on a runner, and it is the
  only way to keep a chat's scroll position, selection and find bar.

## What happens to a chat that isn't shown

- **It keeps running, untouched.** A view closing ends only the view: `unwatch_session` is "for a
  pane that has gone off screen. The session keeps running" (`app/src-tauri/src/lib.rs:2959`,
  `app/src-tauri/src/host.rs:412`). Decision 1 in #4576 needs no change in the core.
- **Its terminal lives in the host.** An `alacritty_terminal` engine keeps 5,000 lines of history
  (`crates/terminal/src/engine/mod.rs:19`, `crates/host/src/hosting.rs:397`), and its output goes
  on being parsed there with no view attached.
- **It holds nothing in the window.** The memory bench says so outright: "a hidden session has no
  pane", and the web view "holds only the panes on screen" (`app/e2e/bench/memory.bench.ts:10-12`).
- **It keeps the size it last had** (`app/src/SessionPane.tsx:382`). A chat that never had a pane
  stays at the 80×24 it started at: `STARTING` is what handoffs and dispatches pass, and "the pane
  it lands in tells it the real one when it is first shown" (`app/src-tauri/src/handoff.rs:1661-1666`,
  used at `:136`, `:220`, `:3438`, `:3516`, `:3810`).
- **It doesn't hibernate for being hidden or idle.** `Cause::Idle` has no caller outside
  `crates/purlis-core/src/state/run.rs` (searched in `app/src-tauri/src` and `crates`). A run reaches
  `Hibernated` when it is stopped by quit or grace (`run.rs:589-590`), or as its host is lost
  (`app/src-tauri/src/hostlost.rs:59-81`). So a chat without a pane costs what a hot chat costs:
  M4, C1 and C2, plus up to M6's history, until the app quits.

## What is kept mounted

- **Only the panes of a project's front tab have terminals.** `PlaneView` draws one `LayoutPanes`,
  for `frontTab` (`app/src/PlaneView.tsx:9863-9870`). Every other tab's panes are unmounted.
- **The last projects are kept off-document.** `KEPT_DRAWN = 4` covers the project in front and the
  ones the person was in last (`app/src/projectShown.tsx:82-86`). Each keeps "the terminals of the
  tab it had in front", in an element that is in no document.
- **So with one pane per tab, the window holds at most 4 terminals.** Splits add one for each extra
  pane in those front tabs. Nothing bounds that sum directly.
- **A kept project's terminals stay live.** Their views stay open and they go on taking output
  (`SessionPane.tsx:61-64`), and they aren't fitted while detached (`SessionPane.tsx:208-212`).

Under #4576, most chats have no tab, so the same rule means most chats have no terminal. That holds
memory down, and it makes the rebuild below the normal path rather than the exception.

## What a switch does today

A pane's chat changing and a tab switching are the same event: `layoutShown` swaps the session in
the same pane (`app/src/tabs.ts:1406-1420`), and `SessionPane`'s effect is keyed on
`[plane, session]` (`app/src/SessionPane.tsx:429`).

1. **Old terminal down.** Listeners go, `unwatchSession` is sent without waiting, and
   `pane.dispose()` drops the xterm and everything it held (`SessionPane.tsx:408-428`).
2. **New terminal up.** `new Terminal`, the fit and search addons, and `pane.open(where)`
   (`SessionPane.tsx:178-262`).
3. **A fit, before anything is drawn.** The xterm starts at its 80×24 default, so unless the pane is
   exactly that, this fires `onResize` and sends `resizeSession` before the view opens
   (`SessionPane.tsx:356-358`, `:384`). On a pane of the size the chat already has, this looks
   redundant: the host and engine take it, and the program shouldn't be signalled, because a kernel
   sends `SIGWINCH` only when the size changes. That last part is from memory of Linux and BSD
   terminal drivers, not checked here.
4. **The view opens.** `watchSession` sends `AttachView` to the host (`hostedsessions.rs:3956-3979`).
   The host builds the snapshot under that session's terminal lock
   (`crates/terminal/src/session.rs:493-526`), walking every line from the top of history
   (`crates/terminal/src/engine/snapshot.rs:66-99`). The cost grows with history and width, and
   that chat's output waits behind the lock while it runs.
5. **The bytes cross.** Chunks of at most 16 KiB (`crates/session-protocol/src/view.rs:45`), held to
   64 KiB in flight (`view.rs:83`), each acknowledged once handed to the pane's channel
   (`hostedsessions.rs:2291-2299`).
6. **xterm parses it all on the UI thread, then paints.** Output waits until the pane has the
   session's size, then it is written and fitted once more (`SessionPane.tsx:396-405`).
7. **A different size means a reflow and a repaint.** If the pane's size differs from the chat's,
   step 3 has already reflowed the engine and told the program, which redraws
   (`crates/terminal/src/session.rs:430-436`). How long a real harness takes to do that has never
   been recorded: the benches use a fake harness.

### Scrollback

Scrollback is sent in full on every switch. The pane is told the cap so it keeps the same history
(`SessionPane.tsx:397-399`). The record suggests this is cheap locally: p50 was 32 ms to a light tab
and 47 ms to one with 5,000 lines of history (ADR 0026), so about 15 ms. Sending the screen first
and history later would save little locally.

A switch also loses what a snapshot can't carry, and all of this is per-chat state a person can see:

- **Scroll position and selection.** They belonged to the disposed xterm, and the new one opens at
  the bottom.
- **The find bar** (`SessionPane.tsx:426`).
- **While a program is on the alternate screen:** the main screen and its scrollback stay gone
  after it leaves (`snapshot.rs:26-29`).
- **Tab stops and character set** (`snapshot.rs:30-34`).

### Focus

Focus is not a cost, as far as the code shows. `showChat` calls `giveKeyboardTo` when the pane's chat
changes (`app/src/PlaneView.tsx:3822`), and the new terminal takes the keyboard in the same effect
that builds it (`app/src/paneKeyboard.ts:56-73`, `SessionPane.tsx:322`), before any bytes arrive.
Typing is sent straight to the program (`SessionPane.tsx:342`), so keys pressed during the snapshot
shouldn't be lost. This comes from reading the code. No test was run.

One trap for the build: the `focused` effect doesn't re-run when only the chat changes
(`SessionPane.tsx:170-172`). Every new route that puts a chat in a pane must call `giveKeyboardTo`
itself, or the keyboard falls to the body.

## Time to a drawn frame

All of this is ADR 0026, M0, 10 samples each, on the operator's machine:

| Switch | p50 | Worst |
|---|---|---|
| To a light tab | 32 ms | 39 ms |
| To a tab with 5,000 lines of history | 47 ms | 48 ms |
| Back to the typed tab, 49 others streaming | not recorded | 41 ms |

`docs/spec.md:561` still carries "worst 48 ms" as L2's last measurement. Five things limit how far
that number reaches:

1. **It predates the session host.** M0 ran in the window's process. Today's path crosses
   `charterd.sock` (`hostedsessions.rs:3956`), and the app runs no chats itself
   (`hostedsessions.rs:11`). The `bench` job holds only the session layer's half of L1 and L3, and
   L2 waits on #931, which is open. So does the release run, #814.
2. **The target is always light and idle.** `load.bench.ts:58-64` switches back to the one quiet
   chat, and `burst.bench.ts:169-210` has one idle heavy chat. Nothing switches to a chat that is
   streaming.
3. **The size never changes**, and the harness is fake, so no real repaint is timed.
4. **It presses a tab button** (`app/src/bench.ts:474-475`). The sidebar pick that #4576 makes the
   main route has no timing.
5. **The 49 streaming chats had no panes.** So L1's 26 ms says nothing about hidden terminals that
   stay mounted and go on parsing.

The nearest thing to a count is `projectSwitch.cost.test.tsx:18-26`, which holds a project switch by
components mounted and commands sent. Nothing like it exists for a pane's chat changing.

## How many can stay mounted

The budgets can't answer this yet, for two reasons:

- **M2 has no measured room.** The web content process is budgeted at ≤ 1.5 GB and marked "at risk,
  expected to miss". Its only reading is a 1,529 MB peak at 12 chats (ADR 0082; `docs/spec.md:548`),
  and M1's 2 GB binds over it. Against that record, any added terminal is unfunded.
- **No bench measures a pane's web cost.** `memory.bench.ts:59-61` says so itself. M6's 20.2 MB is
  the core's copy of a hidden chat, and a kept terminal would hold a second copy of that history in
  the web view.

**Arithmetic, not a measurement:** xterm 6.0.0 stores 3 `Uint32`s a cell
(`src/common/buffer/BufferLine.ts:22`, `:68`), so 12 bytes. At 5,000 lines of history plus a 42-row
screen, 150 columns wide, a full terminal holds about 9.1 MB of cell data. Twenty hold about 182 MB,
which is 12% of M2's ceiling. That leaves out line objects, DOM rows and listeners, and a chat with
short history costs less.

Memory isn't the only limit. Every kept terminal with an open view parses its chat's output on the
UI thread, whether or not it is shown. xterm pauses rendering when it isn't intersecting
(`src/browser/services/RenderService.ts:123-152`), but not parsing. The window drew about 28 MB/s in
total (ADR 0026), and that budget is shared with the pane the person is typing in. Whether the pause
fires for an element out of the document in WKWebView and WebKitGTK is unverified. The project-keep
path already depends on it.

## What must change for switching to feel instant at 20+ chats

**Do these whatever the measurements show:**

1. **Measure the path #4576 creates.** Time a sidebar pick into a focused pane, with 20+ chats
   running, against a target that is idle, one that is streaming, one with full history, and one
   whose size must change.
2. **Count it too.** Add a test in the style of `projectSwitch.cost.test.tsx` for a pane's chat
   changing: components mounted, commands sent, `PlaneView` commits. Counts hold on any machine, and
   this is the only place running-chat count can leak into the switch.
3. **Give L2 a row that says what it is.** Today's row is a tab press to an idle chat at the same
   size. #931 should carry the pick above.

**Depends on those numbers:**

4. **A terminal pool, only if local switching misses or feels slow.** Move the xterm and its view
   out of `SessionPane`'s effect into a window-level store keyed by plane and session, and move its
   element between panes, as `projectShown.tsx` moves a project's host. Keep the last N chats. Past
   N, today's rebuild is the fallback, so nothing else has to change. It would also skip steps 1 to 6
   and keep scroll position, selection and find.
5. **Set N from a measured per-terminal cost, per RAM class.** Don't take it from the arithmetic
   above. A kept chat that streams hard should be let go and rebuilt on return, or it spends the UI
   thread on output nobody is reading.

**Can't be made instant by this:**

6. **A chat changing size.** A pty has one size, so moving a chat to a pane of another size means a
   reflow and a harness repaint, pool or no pool. #4576's rule that a chat is never shown twice
   avoids two panes fighting over it. Dispatched chats meet this on every first showing, at 80×24
   until a pane takes them. The prototype should show what the pane draws until the repaint lands.

**Runner chats need the pool regardless.** At 20 ms round trip, a 2 MB burst took 820 ms at the
64 KiB window (`view.rs:66`), about 2.4 MB/s. A 5,000-line, 150-column snapshot is at least 750 KB of
characters before colours, so about 300 ms by arithmetic. That is three times L2 before xterm has
parsed anything. #930, sizing a remote link's watermarks, is open and would help. A kept terminal
sends nothing at all.

## What the prototype needs to measure

Run with load under 20, on the operator's machine, with a real harness for the rows marked †:

| Measure | Why |
|---|---|
| Sidebar pick to painted frame at 1, 20 and 50 running chats, on today's path | Tests the claim that running-chat count doesn't matter |
| The same to a streaming target, and to a target with full history | Neither has a timing today |
| A pick that changes the chat's size, to the harness's repaint† | Likely the slowest case people will feel |
| First showing of a dispatched chat at 80×24† | The common case under #4576 |
| Web content memory with 1, 5, 10 and 20 retained xterms at full history | Sets N, and replaces the arithmetic |
| Frame gaps and L1 with N kept terminals streaming | The UI-thread cost a pool adds |
| Whether a detached xterm actually stops rendering, on macOS and Linux | Everything off-document depends on it |
| A pick to a chat on a runner, at real round trips | Tests the 300 ms arithmetic |

Each figure should be a median of the job's own samples, as ADR 0086 §1 asks.
