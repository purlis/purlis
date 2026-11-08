# Noted 2026-11-09 (#5824 F7, F8; #5855, #5852): Resolve runs in a folder off to the side

Decided in implementation under the person's standing instruction of 2026-11-08 (take the
recommended path), to be confirmed.

**No multi-step git operation runs at a plane root (#5824 D2), Resolve included.** The window's
Resolve used to leave a real merge, or a replay, stopped at the root. Chats then wrote into a
stopped tree, and a quit left the root mid-merge. Resolve now cuts a **resolve folder**: a linked
worktree of the plane at the state folder's `resolve/`, on the temporary branch `purlis-resolve`,
and the merge is made there.

- **The folder is cut from `merge-tree`, not by `git merge`.** Its files are the tree `git
  merge-tree --write-tree` writes, with what purlis generates already rebuilt, and its index takes
  each conflicted file's three sides as unmerged entries (`update-index --index-info`). No
  `MERGE_HEAD` exists anywhere, so nothing is left for a kill to cut off part-way, while `git
  ls-files -u`, `git checkout --ours` and `--theirs` still work in the folder as in a stopped merge.
- **What the window holds is on disk** (`<git dir>/purlis-resolve.json`), not in the process. A
  resolve outlives a quit: the next look finds it, and Continue no longer refuses with "this window
  did not see where git stopped". A folder without a record is a cut a quit stopped, and the next
  Resolve removes it.
- **Continue is checked, committed off to the side, then one move.** It refuses while a file holds
  a marker, anything is unmerged (#433), or anything outside the files in conflict changed. It
  stages the settled files from the bytes it checked, as a save does, commits the merge with
  `commit-tree` (signed as the plane's saves are), and moves the root with one `reset --keep`.
  `reset --keep` refuses rather than overwrite a file a chat changed meanwhile, and the refusal
  names the file. What was saved at the root since the cut is replayed onto the merge off to the
  side first.
- **Abort removes the folder and touches nothing at the root.** Skip keeps its meaning: a side with
  nothing left to bring is dropped, and the root moves onto the remote's.
- **One merge for every mode.** A push-mode plane's conflict is settled as a merge too, so a
  push-mode Resolve leaves a merge commit where it used to leave a replayed line. *Rejected:* a
  replay stopped commit by commit in the folder, which stops once per commit on the generated
  files (the 2026-11-05 note) and needs a stopped sequencer again.
- **The folder is never staged.** A save leaves it out by name, under each spelling of the state
  folder, so a plane whose `.gitignore` lost its line still never commits it.

**A replay is made off to the side as well (#5855).** A push-mode save's retry and the window's
Update replay the plane's own commits with `merge-tree` and `commit-tree` per commit, keeping each
commit's author, date and message and signing as the save does, and move the root with one `reset
--keep`. They pick what `git rebase` picks: no merges, and nothing the target already has. A
conflict changes nothing, and Resolve settles it in a folder. *Rejected:* `git replay`, which does
not sign.

**A key both machines changed in `charter.toml` is chosen by key (#5852).** The folder's
`charter.toml` is written as the structured merge leaves it, with this machine's value standing in
for each key in conflict and no marker. The window shows each key with its two values and asks for
one. Continue writes the choices and stages the file.

**A fetch that fails while a request is open is said (#5824 F8).** `save-branch.json` keeps `stale`,
since the first failure of the run and the latest failure's words. The standing carries it, and the
save badge says *stale*, with since when and why in its name and tooltip. The next look tries
again, and a fetch that goes through clears it. It is never a block.
