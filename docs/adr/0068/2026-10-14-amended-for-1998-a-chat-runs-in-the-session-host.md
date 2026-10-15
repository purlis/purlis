# Amended for #1998, a chat runs in the session host behind the start-time switch, 2026-10-14

With the start-time switch on, a chat starts, draws, takes input, resizes and stops in the
session host the window started, with the environment and `PATH` it gets in the window. The
switch and both implementations in one build are the operator's ruling of 2026-10-14 (spec
#1974's grill: "both chat implementations may live in one build behind a start-time switch for
the migration only; in-app default until local parity is green, then flip and delete; never a
run-time fallback"). §1, §4, §5 and the amendments above are left as accepted, and this note
adds to them. Items 1 to 3 are **decided by the dispatcher, to be confirmed** (D-1998-1 to
D-1998-3); items 4 to 10 are **decided in implementation, to be confirmed** (D-1998-4 to
D-1998-10).

1. **A start is composed by the binary, and run by the host.** `purlis-host` defines the seam,
   `Compose`, that turns a session protocol start into the program and the environment to run;
   the window's binary and `purlis serve` each implement it with the core
   (`purlis_core::chatspec`), so the host crate still links no core. The window's own path
   composes through the same function, so a chat starts with the same environment and `PATH`
   in either place by construction. What a start is composed of above the window's old seam
   (the sandbox's wrap and the chat's egress proxy, its hooks, its profile's words) stays in the
   window until #1999 and #2000 move what it needs into the host, and travels with the start as
   its **launch**. *D-1998-1, to be confirmed.*
2. **The switch is `PURLIS_CHATS_IN_THE_HOST=1`, read once as the app starts.** It is internal
   to the migration and goes with it (#2005); the default is the window's own process, and
   with the switch off nothing changes. With it on and no host of the window's own (another
   host serves the user, and the window's exited with code 3; or the host did not start), the
   window runs no chats: every start is refused with a sentence saying why (Ruled 3).
   Connecting to a host that is not the window's own is #646's. *D-1998-2, to be confirmed.*
3. **A terminal's view source is built here.** The terminal hands a host a view taken apart:
   the screen as it is now, and an asynchronous stream of what the program writes after it,
   taken together under the terminal's lock, so a view costs a task and not a thread (§4, the
   amendment for #1994). *D-1998-3, to be confirmed.*
4. **Session protocol 1.2, a minor.** It adds, each recorded in the protocol's fixture:
   `hold_project` and `let_go_of_project` (a project by its stable id, with where it is on the
   host's machine); the full start (`start_program` gains the program's id, its run and its
   number, its persona, profile, size, the conversation to resume, its first message, the
   person's opt-out of the sandbox, and the window's `launch`; a shell is the kind `shell`);
   `close` beside `stop` (a stop ends the program and keeps it, with its last screen, until it
   is closed, and answers once it has ended; a close forgets it and answers then, so its number
   is free, and ends it after); `readiness` (the terminal's line discipline and
   how long the program has written nothing, never what it wrote); a started program's number
   and process, and a listed chat's number and run; two pushed notices, `ended` (a program's
   end, with its exit, to every subscriber) and `view_ended` (a view on this link ended for
   good, pushed whether or not the link subscribed, after its stream ends); and the refusal
   code `not_started`. *D-1998-4, to be confirmed.*
5. **A start's `launch`, and holding a project, are the window's alone.** FD-27's table gains
   rows: `close` and `readiness` go with the rows they are like (`stop` and `list`), and
   `hold_project`, `let_go_of_project` and a start carrying a launch are `local-ui`'s only. A
   project's holds are counted per link: a link lets go only of its own, its holds end with it,
   and a project held at one path is not held again at another. A start without a launch, by
   persona, profile and harness, is refused `not_started` until the host composes one from
   those. *D-1998-5, to be confirmed.*
6. **The window deals a chat's number and issues its token.** The hook socket and the chats'
   tokens stay in the window until #2000, so the window deals the number (a chat put back
   keeps its own), announces it before the program starts, issues the token, and binds it to
   the program's process the host names. The host takes the number it is given where no
   program of the project has it, and deals past it otherwise. A hosted program runs below the
   host, which runs below the window, so the window is still every chat's ancestor (HP-6's item
   4). The window names the project to the host by its stable id, minting `[project] id` into
   its `charter.toml` first where it has none (V76). *D-1998-6, to be confirmed.*
7. **Every hosted program is inside a chat to the admission check** (FD-27's item 2): a peer
   below the window that started the host, or below the host itself when no window did, is
   refused every person's scope, and each program is listed by its pid, a closed one's for as
   long as the host runs, since what it started may outlive it in its session. A number kept
   past its session can only refuse a peer, never admit one. *D-1998-7, to be confirmed.*
8. **The host ends its programs as it stops**, so a quit still ends every chat (§2); the window
   ends its host once every project has written its record and ended its chats. *D-1998-8, to
   be confirmed.*
9. **One behaviour suite, two targets.** Each behaviour is written once against the window's
   seam to whatever runs its chats, and run in the window's own process and in the host's
   code over an in-memory session protocol link, with real terminals and the repo's fake
   harness. Until a target runs a real `purlis serve` process over its socket, with the
   environment it inherits checked (#2170), this departs from spec #1974's testing decision
   that the suite speak to a real `purlis serve`; that target blocks the default's flip
   (#2004). Docker and a cluster join later; the in-app target goes with the in-app path.
   *D-1998-9, to be confirmed.*
10. **A pane follows its view across epochs and ends.** A view that comes back at its next
    epoch after falling behind is written a full reset before its fresh snapshot, and a pane
    whose view ended for good (`view_ended`, once its stream has ended) is written the line that
    says its program has ended, as the window's own path writes it. *D-1998-10, to be
    confirmed.*
