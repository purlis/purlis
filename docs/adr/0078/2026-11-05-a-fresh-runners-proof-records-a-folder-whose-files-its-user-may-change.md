# Amended 2026-11-05: a fresh runner's proof records a folder whose files its user may change

**Built by #5112**, following the amendments of 2026-11-03 by #5030 (a large image is looked
through once) and #5103 (the proof counts what its user may write). What follows is *decided in
implementation, to be confirmed* (the D-lines of the pull request that built it).

1. **A folder is recorded in place of its files** where the runner's user may not write the folder
   and may change every regular file in it. Each check reads that folder and looks at its files,
   by the rules the walk that found it went by. What the folder holds can't grow, for the reason
   point 3 of #5030's amendment gives. So an image built with its files made writable for everyone
   is looked through whole, however many such files one folder holds.
2. **One bound, 100,000 things kept**: a folder recorded counts as one, each file recorded by
   itself as one. Past it, the look is said, as before. A folder holding a regular file its user
   may not change keeps its files one by one, so a check never reads a large folder for a few
   files in it.
3. **A link another user owns does not stop a folder being recorded**: its user can't change one,
   and an image's folders often hold them. Nor does a pipe, a socket or a device, which the proof
   never counted.
4. **A check's cost now follows the image**: it looks at every file of each folder recorded, with
   no bound of its own. Only the image sets that, since a task can't add to a folder its user may
   not write. A call that fails keeps the runner.
5. **Rejected:** raising the bound (a check's memory grows with it, and an image can always ship
   more); leaving such files out (a task may change them in place, and that keeps the runner).
