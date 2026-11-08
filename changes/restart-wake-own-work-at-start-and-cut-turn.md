### Fixed

- A chat that started background work in a turn that purlis's quit or update cut off is now told
  about that work when it is reopened. Each background shell, monitor, subagent, workflow, cron
  and wake-up is recorded when its tool starts it, not only when the turn ends. The wake line
  also says that the chat's last turn was ended mid-way and names each piece by what it was.
  `/clear` clears the record, so a cleared chat is never told about work from its old
  conversation.
