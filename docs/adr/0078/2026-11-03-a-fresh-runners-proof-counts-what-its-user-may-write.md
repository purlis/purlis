# Amended 2026-11-03: a fresh runner's proof counts what its user may write, and shows names marked

**Built by #5103**, hardening the two amendments of the same day (#4933's, a fresh runner's
proof survives a host restart and looks beyond its home, and #5028–#5030's). What follows is
*decided in implementation, to be confirmed* (the D-lines of the pull request that built it).

1. **Beyond the home, a file counts where the runner's user owns it or may write it.** Point 4
   of #4933's amendment counted only what the user owns. A regular file another user owns that
   the user may write (by its mode, its group or an ACL, as the kernel answers for the user's
   effective ids) is one a task may change as well, so it counts too: within a place the user may
   write, and outside them, where the one look (#5030) records it beside the user's own files
   and each check stats it again. A link counts only where the user owns it, since changing a
   link it does not own means writing its folder, which is then a place.
2. **A start is refused only where no mark could be made and nothing stands in its place.**
   This replaces point 2 of #4933's amendment. Anything else at `<data>/projects/.first-program`
   (a folder, a link, a pipe, a file purlis can't open) lets the program start: what is there is
   not the file purlis made, which each check says, and that keeps a fresh runner. A host with
   no mark and nothing there still refuses, since started again it would know of no program.
3. **What the proof says is shown marked.** Each sentence of the proof, on the runner and on the
   desktop, is written with every character a reader would not see as its mark, `⟦U+XXXX⟧`, by
   purlis's one rule (`purlis_unicode::marked`, line ends and tabs marked too): a name a runner
   reports is shown as the characters it holds.
