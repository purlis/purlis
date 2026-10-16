# Amended for #2194, the board and Needs you are kept in the session host, 2026-10-16

§1's table puts the board in the host: "It is derived from the hooks, so it lives where they
arrive". Spec #1974 rules that it moves with everything else a chat needs (the operator's rulings
of 2026-10-14). #2000's amendment kept the board in the window, fed by the lines the host hands
on (its item 1). This note replaces that item. What the host keeps is the **kept board**
(CONTEXT.md). Item 1 is decided by the dispatcher as the interim, as #2003's D-2003-6 is. Every
other item is **decided in implementation, to be confirmed** (D-2194-2 to D-2194-7).

1. **The host keeps each held project's board, and the window keeps a copy in step with it.**
   The window still decides each move: what a hook line moves, as its own hearing heard it, with
   what its records say a chat waits on, and the person's acts (an Ignore, an answer, a close, a
   task's report or failure). It makes each move on its own copy and tells the host, in the order
   it made them. The host makes the same move on its own board (`purlis_core::state::kept`). A
   board moves the same way from the same moves, so the two read the same, and none of the
   window's reads waits on the link. *D-2194-1, decided by the dispatcher as the interim.*
2. **Session protocol 1.10, a minor.** It adds `board_moved`, one move as the core says it, and
   `board`, answered `board` with the host's board whole, both as JSON. Both run in one lane per
   project, so a read follows every move sent before it. Only `local-ui` sends either, and FD-27's
   table gains the row. It follows 1.8 (#2003) and 1.9 (#2252).
   *D-2194-2, to be confirmed.*
3. **A project outlives the link that held it while it has a program the window has not closed,
   running or ended.** Its hook channel and its kept board stay, so a window that comes back
   reads how each program ended. Nothing starts, is dealt a number or is confined in it until a
   link holds it again. A project a link lets go of, and one whose programs were all closed, goes
   with its last hold, as before. When a kept project with only ended programs and no link is
   let go of is #2262. *D-2194-3, to be confirmed.*
4. **With no window holding the project, the host moves its board itself.** A line it admits
   moves the board as the window's hearing would, with nothing said of what a chat waits on, so
   the end of a turn is the person's. A program that ends is told to the board with its exit.
   *D-2194-4, to be confirmed.*
5. **A window takes the host's board as it holds the project.** It takes the board whole, before
   it hears any line the host handed on since the hold, and is told every chat on it. From then on
   it tells the host each move it makes. Its count of moves goes on from the highest the host
   kept. *D-2194-5, to be confirmed.*
6. **A board is answered within one control frame.** One larger than the link carries is
   answered as none, the host says so in its log, and the window starts from its own copy.
   *D-2194-6, to be confirmed.*
7. **The behaviour suite.** One behaviour cuts the window's link as a crash would, lets a chat
   end its turn with no window there, and checks that a window holding the project again reads
   every chat's state and Needs you as the host kept them. It runs on the host's target only,
   because the window's own process keeps no board that outlives it. A window started again
   beside a running host is #2006. *D-2194-7, to be confirmed.*
