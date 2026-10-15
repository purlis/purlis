# Your sandbox level is yours, and is kept on this machine

**Accepted 2026-10-15** by the dispatcher, for #2071 (PR B, the personal switch). The choices
below were delegated (*Decided (dispatcher), to be confirmed*). It builds on
[ADR 0067](0067-a-chat-runs-in-a-sandbox-charter-compiles-for-its-harness.md) (the sandbox and
its You level, as amended for `[sandbox].hosts`, #1341) and on the floor of #2071 (PR A), which
makes the floor every chat's default where a project has not turned the full sandbox on.

## The decision

1. **A level is a person's, never a project's.** Each person sets their own level for their
   chats in a project: `on` (the full sandbox), `floor` or `off` (no sandbox). It is kept in
   this machine's `purlis.local.toml`, as `[sandbox] level`, and never in a committed file: a
   committed file may only narrow what a chat is confined to.
2. **Per project, and optionally per persona.** `[sandbox.personas] <persona> = "<level>"` is
   the level for a chat started as that persona, over the project's. Neither set is the floor.
3. **Read only from this machine's own file.** A `purlis.local.toml` git would carry turns
   nobody's sandbox down: its level is read as not set, as its hosts are not read.
4. **Validated where it is saved and where it is read.** A value that is not one of the three,
   or a persona name that cannot be one, is refused by the Settings tab's save with a sentence,
   and the reader skips it.
5. **Where it is set.** Settings › Project › Sandbox › Your level, beside Your hosts. The You
   level is machine-wide and holds no project, so a per-project choice belongs on the project's
   page, written to the local file as Your hosts are.
6. **Before you turn it off, your plain-file vaults.** With no sandbox, a chat can read any file
   on disk, a plain-file vault's included. The same page lists the project's plain-file vaults
   and moves each one this machine registers into the system keychain: every secret is written
   and read back before the registration changes and the file goes, and a move that stops takes
   back what it wrote. A vault the team's file registers is the team's to move, and is refused
   with a sentence. Both commands are the window's alone; the move is a credential command.
7. **A level counts as far as you confirmed it.** A chat can write `purlis.local.toml`: it sits
   in the project, where a sandboxed chat writes. So the file's word alone never turns a
   sandbox down. purlis keeps a record of the levels you set or confirmed in Settings,
   `levels_mine` in this machine's file for the clone, in the person's config home
   (`sandbox-grants/<clone key>.json`, beside Your hosts' confirmations), which no sandboxed
   chat writes. The rule, slot by slot (the project's level, and each persona's):
   - a level the file says that is the one you confirmed, **or stricter**, is honoured;
   - with nothing confirmed, the floor stands for what you confirmed, so `on` and `floor`
     are honoured and `off` is not;
   - a **lower** level the file alone says is shown on the page with **Confirm**, and until
     you confirm it a chat starts at what you confirmed, or the floor;
   - a level the file no longer says is taken off the record whenever Settings reads the
     file, so one taken out by hand and written back later waits for Confirm again.

   Picking a level in Settings confirms it as picked. A save under Edit as TOML confirms
   nothing, since its text may hold a level something else wrote. A Confirm carries the level
   its row showed and is refused where the file says another one by then, so nothing written
   between the showing and the click is confirmed in its place. Both the Confirm and the
   list of waiting levels are the window's alone. A chat that is already unsandboxed runs as
   the person and can write the record too, for this project or another: nothing here holds
   against a chat at `off`, which is what `off` means.
8. **What a start does with it** (2026-10-15, the first wiring, with the floor of #2081). A
   start reads the level only through `sandbox::your_level`: `personal::in_force`, then
   `sandbox::bounded`, the one place a policy bounds it, through `team::allowed_levels` (#2071,
   PR C). An administrator's policy that forbids the opt-out, or the project's
   `[sandbox] policy = "never"`, holds every level at the full sandbox: it lifts `floor` to the
   full sandbox as well as `off`, the level of a chat whose person chose none included. An
   accepted team off (`mode = "off"`) makes no sandbox the level of a chat whose person chose
   none.
   - A confirmed `off`, for the project or the chat's persona, starts the chat as "Start
     without the sandbox" does: every class lifted, audited as the person's
     (`trust.sandbox.off`, `by: your-level`) and counted as an opt-out. Its tab says
     *no sandbox — your setting*, and the folder check an opt-out skips is skipped.
   - **A start another chat asks for** (a dispatch, under a grant or not, or a handoff) runs
     at the stricter of its own level and the strictest level across the chain of chats that
     asked, from the asking chat up to the one the person started, at its first start and
     every later one, so no chat reaches past its own level by asking for another, however
     many hops away. A start that does not say how it was made, or whose chain the record
     cannot name whole, never takes `off`. The levels are read at each start, not when the
     chat that asks was started, so a later change in Settings reaches the chats that running
     chats ask for from then on. The chain is the one the new chat's record keeps, at its
     first start as at every later one.
   - The audit names how the start was made (`made`: local-ui, resume, relaunch, dispatch or
     handoff) and its actor: the person for their own start in the window, purlis for a
     relaunch, a chat for one a chat asked for.
   - A chat that names no persona takes the project's level, never the default persona's.
   - A project that has not turned the full sandbox on starts its chats at the floor (#2081),
     and a confirmed `off` lifts that too, through the same seam.
   - **Every level starts as chosen** (the second wiring): `on` the full sandbox, `floor` the
     floor, in either kind of project. In one that has not turned the full sandbox on, `on`
     starts the sandbox its file would have, with the defaults. In one that has, `floor` starts
     the floor, unless a policy forbids the opt-out, which keeps the full sandbox.
   - An unconfirmed level is measured from the project's own: in a project at the full
     sandbox, a `floor` the file alone says waits for Confirm as `off` does.
   - The title bar shows the level of the chat in front, as its tab does.

## Consequences

- A person who needs their tools to work unsandboxed in one project, or as one persona, says so
  once, on their machine, and their teammates' chats are unchanged.
- The key is new and `experimental` in `docs/plane-format.md` until the start reads it.
