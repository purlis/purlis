# Amended (2026-11-05, #5253; extends "a branch folder is kept current by the app"): a branch folder takes its base by a merge the app runs

A branch already pushed, with a pull request open, is better brought up to its base by a merge
than a rebase: its commits and their review stay as they are. A sandboxed chat could not get one.

1. **It is a flag on `update`, `purlis worktree update --merge`, not a new subcommand.** It does
   what `update` does up to the point where the two differ: the same fetch, the same base, the
   same judging for a sandboxed chat, the same refusals, the same claim on the folder, and the
   same `--continue` and `--abort`. A second verb would have repeated all of that, and given a
   chat two places to look for what to do after a stop.

2. **On the wire it is a step of its own, never a field of the rebase's step.** An app older than
   the command does not know the step and answers nothing. It would have ignored an unknown field
   and rebased, rewriting the commits the chat had asked to keep.

3. **Which files take the base's copy is one fixed rule, never a list a chat writes.**
   `--take-base-baselines` says only whether to, and the rule is a `.png` with a `__screenshots__`
   folder above it, the pictures CI redraws. Where the base removed one, it is removed. What the
   base has there that is not a plain file stays in conflict. Every other conflict still stops and
   names its files. A general `-X theirs` is not offered: it settles every conflicting hunk of
   every file, source included, without a word, and it would still leave a binary's copy to be
   chosen the same way. The rule grows by changing the rule, which is a decision a review reads.

4. **The app runs this outside the chat's sandbox, so it starts nothing the chat could have
   written.** The runner already turns off hooks and every program a repository's config names, as
   it reads that config at each call, whichever file it came from. A merge adds two things no other
   call needed. `branch.<name>.mergeOptions` hands `git merge` options as if typed, a strategy
   among them, so the runner holds that key for every call. And `-s ort` is given outright, so
   `pull.twohead` names no strategy, since a strategy is a program by a name the repository
   chooses. `.gitattributes` only chooses among what config defines, so it reaches nothing once
   that is held. A test plants all of these and watches for marks, and a control shows they run
   for a git nothing holds.

5. **`--abort` is judged too.** It writes back what is staged apart from the commit, so for a
   sandboxed chat a denied path among those refuses it, as the merge's own write would be refused.
   That list is read when the abort runs, because a merge that stood stopped has had a chat
   working in its folder.

6. **Rename detection across a whole folder is off for this merge.** It can write a file of the
   base to a path neither commit's difference names, which the judging of a sandboxed chat's merge
   could not read in advance (#1948). What that costs: a file the base adds to a folder the branch
   renamed stays in the old folder, where it can be seen and moved, rather than following it.

A merge commit takes the message `Merge <base> into <branch>`, and git's notes of the conflict
never reach it. The push's own scan reads what its resolution adds, as it does of any merge.
