# Amended for #2447, `purlis serve` hears hooks and arms with its plugin, 2026-10-17

Spec #2041 (ruled 2026-10-14) asks that a runner chat behave exactly like a local one, and a
runner is `purlis serve`. This note changes what the amendment for #2171 said in its item 10 of
`purlis serve`, and adds to the amendments for #2000 and #2003; everything else stands as
accepted. Every item is **decided in implementation, to be confirmed** (D-2447-1 to D-2447-6).

1. **Both binaries' hosts hear hooks through one channel.** The hook channel of a held project
   (its socket beside the project, its tokens, the check of the process that wrote a line, each
   framed ask read again, its spool, the board kept for it) is the core's
   (`purlis_core::hookchannel`), and the window's host and `purlis serve` each open it as they
   first hold a project. Admission is the same code in both. *D-2447-1.*
2. **`purlis serve` arms a chat with what is installed beside it.** It takes itself as the
   `purlis` every hook runs, purlis's plugin from the place a package or the runner image puts it
   beside its binary (`lib/purlis/plugin`), and writes purlis's git hooks in its data folder as it
   starts, as the window writes them at its launch. With no plugin beside it, a sandboxed Claude
   Code chat is refused as it is wherever the plugin is missing. This replaces "`purlis serve`
   arms with itself and ships no plugin" in item 10 of the amendment for #2171. *D-2447-2.*
3. **The runner image carries the plugin.** purlis's image and the example of a team's image
   copy the plugin of the commit they are built from to `/usr/local/lib/purlis/plugin`, and the
   runner image contract (docs/runner-image.md) asks every runner image for it. *D-2447-3.*
4. **`purlis serve` keeps the event log of the lines it hears**, as the window's host does (the
   amendment for #2003): opened as it first holds a project, written with what the window's
   reply carries, and drained from each chat's spool. *D-2447-4.*
5. **How soon a running chat's spool is looked at is the host's to set.** A host looks at a
   chat's hook spool two seconds after one of its lines is heard, as the window's own hearing
   does (the amendment for #2003); the delay is a parameter of the channel, so a test waits on
   the look, not on the clock. *D-2447-5.*
6. **No wire change.** A held project's hook lines are already pushed to the links that hold it,
   and `reply_heard` already crosses a link relayed for the window alone (the amendment for
   #2042), so the session protocol and FD-27's table are unchanged. *D-2447-6.*
