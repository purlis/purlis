# Noted (2026-10-13, qw346): what section 5 denies, a checkout cannot finish

Section 5 denies a chat the project's manifest and the later-code names wherever it may write.
git checks a tree out file by file, writing each file it may and skipping each it may not. So
an update of the project's root folder from another commit, run inside a chat's sandbox, ends
half done wherever that commit changes a denied file. Measured on git 2.50 with a file the
kernel refuses writing: `git rebase <upstream>` wrote every other file of the upstream's tree,
left HEAD and the index as they were, and left no `rebase-merge`, `REBASE_HEAD` or `ORIG_HEAD`.
Nothing then says an update was tried, and the next save would commit the mix.

Nothing in section 5 changes. Two things follow from it, decided under delegation:

1. **A command that cannot finish does not start.** The Bash guard refuses a sandboxed chat's
   git that rewrites the root folder's working tree from another commit, where that folder
   holds a denied name (`rootupdate`, `docs/hooks.md`). purlis's own saves run inside the
   chat's sandbox too, so their fast-forward, rebase and reset are not started where the
   remote's commits change a file this process may not write (`planegit::sandbox_would_stop`,
   asked of the kernel as `sandbox::held` asks). The window's saves run under no sandbox and
   are unchanged.
2. **The way back stays open.** Putting files back as the current commit has them writes only
   towards it and is allowed. A restore that names a denied file is refused up front, with the
   command the person runs for that file.

Left for a ruling: the guard refuses every such update in the root folder, whatever the other
commit changes, since a `pull` cannot say what it will write before it fetches. And updating
the project is still the window's own doing or the person's terminal: there is no Update
action a chat's refusal could name, and no brokered update a chat could ask for.
