# Noted (2026-10-14, qw387): what a sparse checkout leaves out, the clone guard does not count

The guard of the qw349 note above counts a denied path only where git would write it. Where a
sparse checkout is on in the folder (`core.sparseCheckout`, as git reads it there), git writes
no path its patterns leave out, whatever a move changes, so those are dropped: git's own
`sparse-checkout check-rules` is asked, through the hardened runner, which of the denied paths
they are, only where nothing on the line can change that checkout: every option of the move is
on a short list spelled out whole, no configuration is set anywhere on the line, every git on
it is read (its alias included) and none is `git sparse-checkout`, and no writer comes before
the move. Where git does not answer, every path counts. A command that
cannot finish still does not start.
