### Changed

- **A handoff's refusals name the command a chat can run without being asked.** The heredoc a
  missing or empty brief is answered with, and the line for a workspace that is not there, now
  spell `purlis handoff` with a flag first (`purlis handoff --name "<task>" <workspace>`,
  `purlis handoff --create --vision "<what it is for>" <workspace>`), and no longer say
  `charter`. `purlis handoff report` no longer says "a handoff owes none", which read as a
  contradiction to a chat that owes a report as a task; it says the command sends none and
  names `purlis dispatch report` (#1471).
