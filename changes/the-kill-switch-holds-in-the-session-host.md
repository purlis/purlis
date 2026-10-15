### Security

- **The kill switch holds in the session host.** With chats running in the session host, Stop
  all on the title bar, and `purlis stop --all` once the app hears it, stop every chat the host
  runs; the stop holds when the host starts again, and nothing starts until you re-arm it from
  the title bar. Only the window re-arms; any other client of the host is refused (#2001).
