### Added

- When purlis reopens a chat after a quit or an update, the wake line now says what each piece
  of the chat's own work last put out. For a background shell or a monitor, it quotes the last
  20 lines of output and says where the full output is, or says the output is gone. For a
  subagent, it quotes the last message, and for a workflow, the last thing it reported. A
  wake-up that came due while purlis was down is listed as "was due at <time>, missed". Crons
  and wake-ups that are still ahead are listed as ones Claude Code sets again itself, so the
  chat does not set them twice.
