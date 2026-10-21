# Amended for #2046, code crosses the link as a git bundle, 2026-10-20

ADR 0078 §7 says code moves as git's own pack, carried as one more stream of the link, to a
repository the runner's host keeps. This note says how the session protocol carries that. The
amendments above are left as accepted, and this note adds to them. The minor was assigned on
2026-10-20, and so was the bundle (item 1, D-2046-1, accepted by the steward); the rest is
*decided in implementation, to be confirmed* (D-2046-2 to D-2046-4).

1. **Session protocol 1.34, a minor.** `take_code {project, repo, transfer, refs}` and
   `give_code {project, repo, transfer, have}`, each answered `code {heads}`. The host opens a
   **code stream** for each, as it opens a view: it starts with the kind byte `0x02` and the
   client's `transfer` number as a big-endian `u64` (`purlis_session_protocol::code`). For
   `take_code`, the client writes a git bundle on it and shuts it. For `give_code`, the host does,
   and an empty stream means there is nothing the client lacks. A bundle is held to one GiB on
   either end, and a transfer past that is refused `too_large`. Both commands have a row of their
   own in FD-27's table (`Power::Code`), granted to `local-ui` and to `remote-link` as relayed. A
   relaying host sends them on, and joins the code stream back, for a `local-ui` client alone. A
   host before 1.34 refuses them `unknown_command`.
2. **A hold may name no place** (1.34). `hold_project` without a `project_path` is held where
   the host keeps its own projects, `<data>/projects/<project id>` on a runner, made there if it
   is not. A project id that is not a ULID is refused `not_allowed`, and a host that keeps no
   projects refuses it `no_such_project`. *Decided in implementation, to be confirmed
   (D-2046-2).*
3. **Code moves only in a project held where the host keeps it.** A project held at a place a
   client named takes no code, so a desktop's transfer never writes anywhere on a runner but
   the runner's own folder for that project. *Decided in implementation, to be confirmed
   (D-2046-3).*
4. **A bundle on its own stream, not on the control lane.** The control lane goes ahead of
   every stream (ADR 0068 §4, amended by FD-4), so a large bundle there would hold up every
   Needs you behind it. A stream takes turns with the views, under its own credit. *Decided in
   implementation, to be confirmed (D-2046-4).*
