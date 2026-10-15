# Amendment, 2026-09-27: the plane root is anywhere in the plane outside every workspace (SI-1b)

The amendment above made the plane root a place, and put there exactly the chats started in the
plane's own directory. A chat started in `docs/` was filed on the plane root's tab — the window
files every chat in no workspace there — but was told neither variable, so its own ladder fell
to the plane's default workspace and asked the operator which workspace it was in. The
operator's ruling: **any directory inside the plane but outside every workspace is the plane
root.**

- **The app marks every chat it starts there** with `$PURLIS_PLANE_ROOT_SESSION=1`: the plane's
  own directory, `docs/`, `.charter/` and `workspaces/` itself. Outside the plane it still marks
  nothing.
- **`purlis` treats such a directory the same way when nothing else speaks for the session**
  (`active::plane_root`, the one question the CLI, the hooks and the footer all ask). The rungs
  that speak for a session — `-w`, `$PURLIS_WORKSPACE`, a workspace's tree underfoot, the
  session's pointer, its terminal's — still answer first. What no longer answers for it is the
  plane's two defaults, `workspaces/.default` and `[workspace] default`.

**Why the defaults give way, and the pointers do not.** The defaults are not about any session:
they are a committed line saying which workspace a caller that is *nowhere* gets — a script
run from outside the plane with `$PURLIS_ROOT` set, a job with no directory of its own. A
session standing in the plane is somewhere, and the window has already drawn it on the plane
root's tab; letting a plane-wide default speak for it is how that chat was quizzed. So the
defaults keep their whole meaning outside the plane, and `active::workspace` asked on its own
still ends on them. The pointers are different in kind: each was written by a
`purlis workspace use` in that session or that terminal, which is somebody choosing a
workspace for exactly this caller. They stay above the plane root, so a session nothing pinned
to the root still moves with `workspace use`, as it always has — only a chat the app *started*
at the root refuses to be moved.

**What it costs.** Python answered the plane's default for a session at the plane root with no
pointer, and a terminal session there that ran `purlis ws todo "…"` with no `-w` wrote into
that default. Now it is asked to pass `-w` (or run `purlis workspace use`). The recorded
scenarios that stood at the plane root with no pointer and needed a workspace moved on purpose,
each with a note saying so (ADR 0046).

**Three things said about a root chat that still named a workspace now say the plane root:**
its footer (`⬢ plane root`, with the environment pin `*` when the app put it there, and no
workspace's counts), its handoff's stamp (`⟨handoff from chat 16 · plane root · …⟩` rather
than the ladder's workspace — the chat it opens still starts in the workspace the brief names),
and where a report back to it is kept once it has closed (`.charter/handbacks/plane-root/`, which
the next chat started at the plane root reads). On disk and on the wire the plane root is the
two words `plane root` wherever a workspace's name was (`active::Place`): no workspace name can
have a space, so every reader that held those fields to the name rule refuses the value rather
than joining it onto `workspaces/`.
