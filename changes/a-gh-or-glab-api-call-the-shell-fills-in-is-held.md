### Security

- **The merge hold no longer lets a `gh api` or `glab api` call that the shell fills in merge.**
  A call with no method on the line was read as a read whatever its arguments, so
  `gh api repos/o/r/pulls/5/merge $(cat n)` merged when `n` held `-XPUT`, and a merge request's
  title or a user's bio could hold it. Such a call is now held, read or not, when an argument
  has an unquoted `$…`, `$(…)` or backtick, or begins with an expansion. A variable the line
  binds to digits, and an expansion in double quotes after a literal start of its word
  (`"repos/o/r/pulls/$N"`), still pass. The refusal says so.
