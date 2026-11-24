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
