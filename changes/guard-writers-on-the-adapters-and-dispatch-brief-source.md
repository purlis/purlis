### Changed

- **Each harness says where `purlis guard` writes its rule.** The writer for a project's ask or
  allow rule is now part of each harness's adapter, instead of a list of harness names inside
  the guard command. What `purlis guard ask|allow` writes and prints is unchanged (#922).

### Security

- **A dispatch's brief is held to the same rule as a handoff's.** In a Claude Code chat, a
  `purlis dispatch` whose brief does not come from one quoted heredoc in the same call is now
  asked about. That covers a pipe, a file, a here-string, an unquoted heredoc, or no brief at
  all (#1471).
- **A redirection that writes a file beside a dispatch or a handoff is asked about.** It is
  treated like a second command in the call. A duplicated descriptor (`2>&1`) and `/dev/null`
  are not asked about (#1471).
- **`purlis handoff report` is read the way the command reads it.** With no summary after
  `report`, only redirections, flags or a bare `--`, the call is a handoff into a workspace
  called `report`, and its brief is judged (#1471).
