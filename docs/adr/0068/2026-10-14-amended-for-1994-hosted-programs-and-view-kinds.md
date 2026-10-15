# Amended for #1994, hosted programs and view kinds, 2026-10-14

The session host holds **hosted programs**, each shown through **views** of a kind: ruled by the
operator on 2026-10-14 (the simplification round of spec #1974's grill, "agree at all"). A
chat's harness and a shell are hosted programs, and a browser is one from spec 2 (#2008); a
terminal view is one kind of view, and a browser view another. Start, attach, detach, reattach,
stop, the kill switch, the journal and the upgrade handoff are written once for hosted programs,
so a browser inherits them. §4 and the FD-26 amendment above are left as accepted, and this
note adds to them. What the reattach prototype of 2026-10-14 had to work around is what it adds.
What the ruling settled is the model and the phrasing of the commands. Two details are
**decided in implementation, to be confirmed**: the exact wire words and the refusal of an
unknown kind in item 1 (D-1994-6), and how a reader is told the snapshot is complete, in item 4
(D-1994-1).

1. **The commands say "start a hosted program" and "attach a view of kind X", never "start a
   chat".** Session protocol 1.1, a minor, adds `start_program` (a `kind`, `harness` or
   `shell`, with the project and optionally a workspace and a harness), answered
   `program_started` with the program's id; `attach_view` (a `program` and a view `kind`,
   `terminal`), answered `attached` with the view's `kind` beside its number and size; and
   `detach_view`. Each is recorded in the protocol's fixture. 1.0's `start`, `attach` and
   `detach` are carried out as these and answered in 1.0's words, so a client one version
   behind is unchanged and a host hears only the hosted-program commands. `start_program` is
   granted as `start` is, and `attach_view` and `detach_view` as `attach` and `detach` are
   (FD-27's table). A kind a host does not know is refused `malformed`. *The words and that
   refusal: decided in implementation (D-1994-6), to be confirmed.*
2. **A view of any kind behaves the same, and the crate holds it.** A hosted program offers a
   view **source**: each attach hands over its snapshot apart from the live bytes after it,
   taken together so nothing is lost or doubled, and the live bytes are asynchronous, so a view
   costs a task and not a thread. A link's views are numbered, followed and closed by the
   protocol crate, not by each host. A view that falls behind is dropped and attached again at
   the next epoch from a fresh snapshot, taken after the pause a client that drew nothing waits
   out, whatever its kind. A kind brings only its snapshot and
   what its bytes mean: a terminal's screen and scrollback, a browser's latest frame.
3. **A link that ends closes its views at once, and the host is told.** When a client closes
   its link or dies, every view on that link is closed straight away, before the host hears of
   it, and the programs run on. A new client attaches as any client does, and its snapshot is
   the program as it is now.
4. **A reader is told when the snapshot is complete.** The view's header carries the
   snapshot's length, so the reader says when every byte of it has come, at once for an empty
   one. The view's stream is unchanged on the wire. *Decided in implementation (D-1994-1), to be
   confirmed.*
