### Security

- **The guards read a few more shapes, and none fewer.** `watch`, `flock`, `script`,
  `parallel`, `caffeinate`, `arch` and `gtimeout` are read through to the program they run, and
  where one hands a shell a string instead (`watch 'a | b'`, `flock -c`, `script -c`), the
  string is read as `sh -c '…'` is. An ask or deny rule that opens with a wildcard and then
  names its program (`*kubectl delete*`) holds under every spelling of that program, and the
  rules in your own Claude Code settings (`~/.claude/settings.json`) are held as the project's
  are; purlis reads that file and never writes it. A heredoc whose substitution closes on a
  later line into a pipe to a shell is read as a script that shell runs. A handoff's brief fed
  by `xargs` or a duplicated descriptor (`<&3`), and a `report` that carries a handoff's own
  flags, are judged as briefs; a handoff that carries the hidden `--now` is asked about. A few
  more forge CLI commands that publish text (gh's `issue`/`pr` `close`/`reopen --comment`,
  `project item-create`/`item-edit`, `agent-task create`; glab's `incident note` and
  `work-items create`/`update`) refuse a live substitution as `gh issue create` does
  (#799, #1096, #1325, #1471).
