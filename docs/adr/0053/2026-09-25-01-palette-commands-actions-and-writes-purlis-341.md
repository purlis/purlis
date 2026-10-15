# Amended 2026-09-25: palette commands, actions and writes (purlis#341)

The first three real capabilities, built together because each needs the others to be useful.
The approval prompt names each one and then lists what it declares, one line each.

- **`palette`** adds commands to the palette (`contributes.palette`: `id`, `title`, and exactly
  one of `view` or `action`, each one the same manifest declares). The palette names a command
  `<extension's name>: <title>`, so where it came from is on the row.
- **`actions`** puts the extension's own actions on the rows of its views
  (`contributes.actions`: `id`, `title`, `confirm`, and optional `deletes`). An answered row
  names the actions it offers by id. The executor refuses an answer whose row names one the
  manifest does not declare. The button's title and whether purlis asks first come from the
  manifest, never from the answer.
- **`writes`** declares the plane paths the extension writes (`contributes.writes`:
  plane-relative globs, `*` and `?` within one segment). A path that starts with a pattern,
  names anything hidden, or covers a file purlis reads settings, grants or vaults from
  (`charter.toml`, `charter.local.toml`, `vaults.json`, a workspace's `workspace.json`) is
  refused at parse.

**A capability's shape is under its own word, and is there exactly when the word is asked
for.** A manifest with `contributes.actions` but no `actions` in its list is refused, and so is
one that lists `actions` and declares none, empty list included. This is the pairing check
purlis#340 built (`Capability::has_shape`), and every shaped capability keeps it.

**The protocol is 2**, because an action is a second kind of request: *run action `<id>` on
`<subject>`*, with the view and the row it was pressed on, and an answer that may carry the view's
blocks refreshed. Every protocol-2 request also carries `writes`, the declared paths resolved
against the plane. The two companion changes this record called for are made. The executor
asks each program in the protocol its manifest names and reads its answer in that protocol. The
fingerprint hashes the manifest's protocol, not the executor's. A protocol-1 extension such as
persona statistics is asked the same bytes as before and keeps its approval: a test pins its
fingerprint. `actions` and `writes` need protocol 2, so a manifest that names version 1 and asks
for either is refused. `palette` needs only 1.

**Confirmation is purlis's, and a delete always asks.** An action says whether purlis asks
first (`confirm`, required). It also says whether it deletes (`deletes`, default false). An
action that deletes is asked about whatever `confirm` says. The executor refuses to run an
action that asks first without the operator's yes, so a window that forgot to ask gets a
refusal, not a delete. **How purlis knows an action deletes is that the manifest says so.** An
extension that deletes without saying so skipped the question. The write report catches that
case: a path deleted by any question except an action declared as deleting is reported, even
inside the declared paths.

**The write report is detection** (ADR 0041). Before the program starts and after it stops, the
executor lists what `git status` shows in the plane, with each path's size and modification
time. It reports a change outside the declared paths as a sentence naming the extension, beside
the answer or the refusal. It sees what git would commit. It does not see an ignored path, a
write that keeps size and time, or a plane that is not a git repository. It cannot tell the
extension's write from a chat's in the same moment, and the sentence says so.
