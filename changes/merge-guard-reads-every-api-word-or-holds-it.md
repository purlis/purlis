### Security

- **The merge hold passes a `gh` or `glab` call only when it is plainly a command purlis knows.**
  Every other call is held, so a command the list forgot costs a false hold, never a merge. A
  call passes when every word up to its verb is a plain, non-empty word (no `$`, glob, brace,
  `~`, `#`, `^`, and not `""`), every flag before the verb takes no value or is the
  repository's (`-R o/r`, `--repo=o/r`), and its noun and verb are a read or an ordinary
  non-merge command purlis lists (`gh pr view`, `glab mr list`). `gh api` is read word by word
  as before. Behind a program that never runs its operands (`cp`, `grep`, `git log`, `brew
  install`, `which`), `gh` is only a name. Behind any other program (`fd -x`, `chronic`, `ssh h`,
  `brew bundle exec`, `mise exec --`), `gh` or a path to it is a call that the program may
  give more words, so it is held unless its noun and verb are already written out.
  A `gh api` read behind such a program, or a mistyped `gh push`, is now held too. The refusal
  says to write the noun and the verb out and put the flags after them. The text guard is an
  early warning; under `guards.merge = enforce` the forge route is the boundary (#7423).
  A flag before a package manager's or `git`'s subcommand (`git --namespace log bisect run gh …`, `npm --loglevel info exec -- gh …`) no longer makes the call names-only, and a `find -exec gh -R o/r +` that `find` may complete with file names is held.
  **Round 13: the programs behind which `gh` is only a name are a minimal list** of those that
  provably cannot run a word: `cp`, `mv`, `ln`, `rm`, `ls`, `stat`, `file`, `du`, `cat`, `head`,
  `tail`, `wc`, `diff`, `cmp`, `shasum`, `md5`, `md5sum`, `sha256sum`, `grep`, `rg` (not with
  `--pre` or `--pre-glob`), `mkdir`, `touch`, `chmod`, `chown`, `which`, `type`, `man`, `echo`,
  `printf`, and `git` for `add`, `status`, `log`, `diff`, `show`, `blame`, `shortlog`, `grep`
  (not `-O`), `mv`, `rm`, `branch`, `tag`, `fetch` and `remote`, with no flag before the
  subcommand but `-C <dir>` and `-c <key=value>`, and no `-c` key or `GIT_*` assignment that
  names an editor, pager, hook or ssh command. Package managers, `sed`, `awk`, `find`, `less`,
  `tee`, `jq`, `sort`, `install`, `codesign`, `xattr`, `locate`, `mdfind`, `apropos`, `tldr` and
  `git commit`, `checkout`, `switch`, `restore`, `reset`, `stash`, `pull` and `push` are not on it,
  so `brew install gh` and `git commit -m fix gh` are held now (a false hold, accepted): run the
  `gh` command first, on its own, or say what you want done. `find` ends a `-exec`, `-execdir`,
  `-ok` or `-okdir` run at `+` only directly after `{}`, as find does, so `find . -exec env -u +
  gh pr merge 5 \;` is read whole. A command inside ONE quoted word that a program other than a
  shell runs (an editor or pager config value, `awk`'s `system()`, `GIT_EDITOR`) is not read
  (`git commit -m "mention gh pr merge in text"` passes); under `guards.merge = enforce` the
  forge route is the boundary for those.
