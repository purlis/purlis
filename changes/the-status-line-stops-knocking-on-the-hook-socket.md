### Fixed

- **The app's log no longer fills with refused hook connections.** Claude Code's footer in an
  app chat used to connect to the app's hook socket each time it refreshed, only to see whether
  the app was there, and each of those connections left a line in the app's log. That was about
  13,000 lines a day. The footer now checks that the socket's file is there and does not connect.
  A connection that closes before the app can see who made it is counted quietly instead of
  logged.
