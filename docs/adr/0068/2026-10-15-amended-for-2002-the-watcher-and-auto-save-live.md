# Amended for #2002, the watcher and auto-save live in the session host, 2026-10-15

With the start-time switch on, the session host saves each project the window opens: it runs
the plane's watcher and auto-save, and the window starts none of its own. Spec #1974 puts the
watcher and auto-save in the host with everything else a chat needs ("the cut is above today's
seam", user story 25); ADR 0051's "auto-save lives in the app, because there is no daemon" holds
with the switch off and is amended by this note with it on. The items are **decided in
implementation, to be confirmed** (D-2002-1 to D-2002-7).

1. **Saved where the chats run, by the same code.** `purlis-host` defines the seam, `Save`, as
   it defines `Compose`: the window's binary implements it with the plane's watch and the same
   auto-save worker the window runs, so the host crate still links no core, git or forge code.
   A save in the host commits with the identity, scans and hooks the window's would, since the
   host is the window's binary run with the window's environment, and it looks at a request
   with the person's sign-in where there is one. `purlis serve` gives no saver, and refuses to
   save with a sentence. *D-2002-1, to be confirmed.*
2. **Session protocol 1.7, a minor.** It adds `start_saving`, `stop_saving` and
   `fetch_project` (a project by its stable id, held on this host); the pushed notice `saving`
   (the project, whether the look `saved` it, and whether what it did `moved_tree`); and the
   host's question to the window of who is mid-turn, the pushed `who_is_mid_turn` (a numbered
   `question`, the project and a workspace), answered with `mid_turn` (the `question`, and the
   chats, or none said where the window cannot tell). The commands go with `hold_project`'s row
   of FD-27's table: `local-ui`'s only, and the question is pushed to the window's links alone.
   Each is recorded in the protocol's fixture. *D-2002-2, to be confirmed.*
3. **Exactly one saver per project, and one save at a time.** The host runs at most one saver
   per project however often it is asked, and the window, whose chats run in the host, starts
   none: what saves a project is chosen at the same seam that chooses where its chats run.
   Every save and fetch of a repository, from whichever process, holds a lock in its git
   directory (`purlis_core::planegit::Claim`, `<git dir>/purlis-save.lock` in
   `docs/plane-format.md`), so the window's Save, a resolve, a repo's save, the rename fix,
   `purlis save` in a terminal and the host's auto-save never run at once: a second is refused,
   or waits where it may. With the switch on and no host of the window's own, the project is not
   saved by itself on that launch, and the log says so (Ruled 3: no fallback at run time).
   *D-2002-3, to be confirmed.*
4. **A link that ends is not a stop.** A window gone without a quit leaves its projects being
   saved: the host's saving ends only with the window's `stop_saving`, with every window
   letting go of the project (`let_go_of_project`), or with the host. A window that comes back
   and asks again finds the same saver. Today the host ends with the window's process (the
   amendment for #1997, item 5), so a change made after a crash is saved for as long as the host
   outlives its link; the grace period after a crash, and when the saving of a project nobody
   holds stops, are #646's. *D-2002-4, to be confirmed.*
5. **What the window is told.** Each look that did something is pushed as `saving`; the window
   reads its git standing again, or everything when the tree moved, and tells the extensions
   that hear a save, as it did when its own worker told it. Focus asks the host to fetch
   (`fetch_project`); a program's end in the project is the host's own to hear. The host's
   watch never looks for itself, since no window is beside it: its events, and the standing read
   again after `planegit::SHARED_FOR`, keep its auto-save current. The window keeps its own watch
   for its panels. *D-2002-5, to be confirmed.*
6. **Who is mid-turn.** A repo's auto-save waits while a chat that could be writing its
   workspace is mid-turn (ADR 0051), and the turns are on the window's board until the live
   board moves into the host (#2194). So the host asks the window (`who_is_mid_turn`), which
   answers from its board as its own worker asked it. Only with no window linked, or one that
   does not answer within two seconds or cannot tell, does every program the host runs in a
   place that could be the workspace count as mid-turn: such a repo is then saved only while no
   chat runs there, or when one ends, erring towards saving later and never towards committing
   a turn's half-written file. *D-2002-6, to be confirmed.*
7. **A quit is bounded, and its save never overlaps the host's.** A quit stops every project's
   saving at once, against one deadline of three seconds; the host answers a stop once its
   worker has let go of the plane, a save it was in the middle of included, for at most two.
   Where it does not answer by the deadline, the window ends the host before its own save at
   quit runs. The save at quit then waits on the plane's lock as any save does. *D-2002-7, to
   be confirmed.*
