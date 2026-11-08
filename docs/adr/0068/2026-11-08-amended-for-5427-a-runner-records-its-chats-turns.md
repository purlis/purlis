# Amended for #5427, a runner records its chats' turns, 2026-11-08

A chat on a runner or a Docker pool recorded no turns (#5405, decision g), so the file diff had
no turn of it and **Undo a turn** (ADR 0081, amended 2026-11-05) was never offered on it. The
operator ruled on 2026-11-08, option (a): the runner's host records the turns where the chat
works, the desktop fetches them over the link beside the runner's branches, and an undo is a
request the runner's host carries out under its own idle check. This note says how the session
protocol carries that. The amendments above are left as accepted, and this note adds to them.
Items 1 to 3 are the ruling's; the names in items 2 and 4 are *decided in implementation, to be
confirmed*.

1. **Session protocol 1.78, a minor** (`KNOWS_TURNS`). A host before 1.78 records no turn of a
   chat on it: the window asks it for none and sends it no undo, and the turn diff says the
   runner is too old to record its turns, where it showed no turn before. Undo is then never
   offered, as no turn of such a chat is ever here.
2. **`give_code` may carry `with_turns`.** The host then names every
   `refs/purlis/turns/<chat>/<n>/{start,end,undone}` the repository holds among the `heads` it
   answers, and bundles them with its branches. The window takes only refs that are turns as
   purlis writes them (a plain chat id, a number, one of the three sides), refuses the whole
   fetch on any other, and keeps them as `refs/purlis/runners/<pool>/turns/…`: beside
   `refs/remotes/<pool>/`, not inside it, since a fetch prunes that folder to the runner's
   branches and lists every ref in it as one (`docs/plane-format.md`). A copy the runner no
   longer names goes. A host before 1.78 ignores the field. *The copies' place: decided in
   implementation, to be confirmed (D-5427-1).*
3. **`undo_turn {project, workspace, repo, piece?, chat, number}`**, with a row of its own in
   FD-27's table under `Power::Code`, granted to `local-ui` and to `remote-link` as relayed. The
   host carries it out in a project it keeps itself, by that project's hook channel, which hears
   its chats and keeps their board: while that board says a chat working in the folder is not
   idle (ADR 0084 §8), or the chat is still recording its last turn, it answers
   `undo_waits {chats: [{number, state}]}` and writes nothing; otherwise it puts the turn's files
   back through the core, as the window does for a chat of its own, and answers
   `turn_undone {edit, unmarked?}`. A turn that is not the latest not yet undone, or whose files
   changed since it ended, is refused `not_allowed` in the core's words, naming the file. A
   boundary of a turn heard during an undo waits until it is done. The window keeps `edit` as
   the human edit (ADR 0084 §8), tells the chat at its next turn, and marks its copy undone.
4. **The runner's host records turns on the hook path it already hears.** The core's turn
   recorder moved from the app into `purlis_core::files::turns::Recorder`, and `purlis serve`'s
   hook channel records with it (`hookchannel::HostedTurns`) under each chat's id and folder as
   the host composed it. A chat's turns go when its host forgets it, as the window removes those
   of a chat of its own when it ends it; the window removes its copies at the same point. *The
   recorder's home: decided in implementation, to be confirmed (D-5427-2).*

**When the copies arrive.** With the runner's branches: at a chat's end, and on autosave's fetch.
A turn that ended on the runner since the last fetch is not in the diff until the next one.
