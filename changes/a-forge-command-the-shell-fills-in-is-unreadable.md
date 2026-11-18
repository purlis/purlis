### Security

- **A forge command whose program, noun or verb the shell fills in is held.** `$(echo gh) pr
  merge 12`, `gh pr $(echo merge) 12`, `$GH pr merge 12`, `git $(echo push) --tags` and their
  backtick, `${…}` and glued spellings passed the merge hold and the release floor, since
  neither read a `gh pr merge` or a `git push` in them. A program the shell fills in is now read
  as each CLI it could be, and a noun or verb that could name a merge or a publish is read as
  one purlis cannot read. An argument after a verb that is read, such as `gh pr view $(cat n)`,
  still passes (#7124).
- **Moving the default branch through the API is held as a merge.** `gh api -X PATCH
  repos/o/r/git/refs/heads/main -f sha=…`, its `DELETE`, and a new `refs/heads/main` made with
  `POST git/refs` move `main` with no request, and were let through. A write to `main` or
  `master`, or to a ref whose branch purlis can't read, is now held in every mode; a feature
  branch's ref is not (Q330).
- **More routes to the default branch and to a tag are held** (#7131). A ref path gh fills in
  (`git/refs/heads/{branch}`), a GraphQL mutation that touches a ref, a branch or a tag
  (`updateRef`, `createRef`, `createCommitOnBranch`, branch protection), a program the shell
  builds (`$(echo gh pr) merge 12`, `{gh,} pr merge 12`, `g[h] pr merge 12`), renaming `main`,
  making another branch the default, GitLab's branch delete, commit and protection endpoints,
  `git push origin HEAD:refs/tags/v1`, and a `git push` flag whose value the shell fills in are
  now held or refused unattended. So are a brace that builds a program from a path
  (`{/opt/homebrew/bin/gh,pr,merge,12}`), a substitution that prints a command before a path
  (`$(echo gh pr merge 12)/y`), zsh's `=gh`, glab's `:branch`, a GraphQL mutation whose input
  comes in a variable or that deletes a ruleset, a ruleset write, GitLab's file, cherry-pick and
  revert commits to `main`, and `git push --mirror`, `tag <name>`, `refs/*:refs/*` and
  `-c push.followTags`. A `case` pattern, an arithmetic expansion, an array's words, `"${cmd[@]}"`
  and the body of a `python3 -`, `node -` or `ruby` heredoc are no program the shell builds, and
  pass. This reading is an early warning; the forge credential is the lock (#7133).
