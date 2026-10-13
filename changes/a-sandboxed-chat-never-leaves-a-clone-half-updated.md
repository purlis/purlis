### Fixed

- **A sandboxed chat can no longer leave a clone or a branch folder half-updated.** A chat's
  sandbox never lets it write the files a later program loads code from (`.vscode/`,
  `.husky/`, `.claude/settings.json`, `.mcp.json`, `.envrc` and the like), in any folder, and
  many repositories track one. A `git pull`, `rebase`, `merge` or `reset --hard` whose other
  commit changed such a file wrote every other file and failed; a `git checkout` or `switch`
  did the same and reported success, with that file left behind as if the chat had changed
  it. Before such a command runs, purlis now asks git which files it would write and refuses
  only one that would write a file the sandbox denies. The refusal names the files and says
  the person runs the command in their own terminal. A move that touches none of them runs
  as before.
- **`git pull` in a repository that tracks such files is told to fetch first.** A pull
  fetches the commit it moves to, so it cannot be checked before it runs. In a repository
  that tracks one of those files the refusal says to run `git fetch`, then `git merge
  --ff-only @{u}` or `git rebase @{u}`, each as a command of its own, which purlis checks.
  A move with a command that may write before it on the same line (`git fetch && git rebase
  origin/main`, `gh pr checkout 7 && git merge main`) is told to run as a command of its
  own. In a repository that tracks none of them, nothing changes.
- **A `git clone` of a repository on this machine that tracks such files is refused up
  front**, with `purlis clone <repo>` (the app makes the clone, whole) and the person's
  terminal as the ways through. A clone from a URL cannot be asked before it is fetched.
  A checkout in a folder cloned with `--no-checkout` is refused the same way, since it
  writes the whole tree.
- **A chat is told when git stopped partway all the same.** After a git command that said it
  could not write one of those files, the chat is told in that turn that the folder is
  half-updated, not to commit from it, and the command that puts it back or finishes it.
- **A long line of `cd <folder> && git …;` no longer makes the Bash guard slow.** Every git
  on a line is read against each folder the shell may be in, and each such `cd` left one
  more, a level deeper, which the guard then asked the disk about again for every git: a
  16 KB line of them took the project-root guards 27 seconds. Each folder is now asked
  about once for the line, and past 64 folders, or 8 KiB of their names, none is spelt: a
  git there is one the guard cannot place, which it refuses as it would in the root.
