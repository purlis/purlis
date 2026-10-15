# Noted (2026-10-14, qw382): a blocked save is re-derived, and its way out is never a loop

Decided under delegation, and recorded for the operator's ruling.

- **Save re-runs a push that stopped.** On a plane whose last push stopped in a conflict or a
  stranded commit and has not landed, Save re-runs the push and its fetch-and-rebase retry
  even when the tree is clean and `[plane] mode` is unset. The answer it leaves is current:
  the push lands and the record goes, or a conflict is recorded again with its files, or git's
  own words are.
- **A conflict that names nothing is not believed.** A `conflict` record with no `detail` and
  no files is no block. The Saving view says "Save stopped earlier — Save to retry", and the
  next save, automatic or pressed, retries the push.
- **Update forgets what its replay settled.** After the window's Update replays the project's
  own commits onto the remote's, the push record about the commit it rewrote is deleted.
- **A real conflict names its files and is settled in a terminal.** The title bar and the
  Saving view name the files. A chat may not rebase the project's root, so a conflict block
  offers no chat. Its Notice gives the git commands (`git fetch`, `git rebase @{u}`, `git add`,
  `git rebase --continue`, `git rebase --abort`) and *Open terminal here*. Update's own
  conflict answer gives the same commands.
- **A chat is offered by what git stopped in.** Where git is stopped part-way in a rebase,
  `git am`, a cherry-pick, a revert or a bisect, finishing it is git's in a terminal, so no chat
  is offered, whether or not files are still unmerged. A stopped merge, or unmerged files with
  no operation, still offer one.
- **The window's Save carries what the remote lacks.** Pressed in the window, Save on a clean
  plane pushes the commits its upstream lacks even when `[plane] mode` is unset, so the window
  never says "nothing to save" while there are some. `purlis save` keeps its recorded answer.
