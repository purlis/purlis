### Fixed

- **A brokered `purlis secret exec` keeps its output whole with a long value.** In a sandboxed
  chat, a vault value longer than about 190 KiB could make the app send a piece of output larger
  than the command reads, and the command then said the app had stopped answering. Output now
  goes back in pieces that always fit, masked as before (#1407).
- **A brokered run says how many more hosts were refused.** The sandbox names the first eight
  hosts it refused a brokered command; past those, the command is now told how many more there
  were, instead of nothing (#1353).
- **A chat whose spool key could not be recorded at its start gets it recorded later.** When
  another process held the hook spool's lock as a chat started, its key was never recorded, so
  every hook line that chat spooled while the app was busy was rejected. The key is now recorded
  at the next chance (the chat's next hook line, or the next chat to start), while the chat
  still runs on that key (#1426).
