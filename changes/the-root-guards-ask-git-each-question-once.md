### Fixed

- **A long command with many `git` calls is judged in time on macOS.** The guards that keep the
  project root on its branch and its unpushed commits safe asked git the same question again for
  every `git` on the line, each a git process. A long line of resets and restores could take them
  past the hook's two seconds, and the command was refused for running out of time. Each question
  is now asked once per command, and the answers are the same as before (#2619).
