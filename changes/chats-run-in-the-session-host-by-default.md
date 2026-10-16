### Changed

- **Chats run in the session host by default.** Every chat now runs in purlis's session host
  rather than in the app's own process, so a crash of the app no longer ends it. Nothing you see
  or do with a chat changes. Upgrading loses nothing: each chat that was open reopens on its own
  conversation, on its own profile, in its own folder and branch, and your session records and
  the profiles you declared yourself work as they did, with nothing to set up or approve again.
  Quitting still ends every chat, and so does an update. If chats misbehave in the session
  host, you can run them in the app again while purlis still carries both: on macOS run
  `launchctl setenv PURLIS_CHATS_IN_THE_HOST 0` in a terminal, then quit purlis and start it
  again (`launchctl unsetenv PURLIS_CHATS_IN_THE_HOST` and a restart undo it); on Linux start
  purlis with `PURLIS_CHATS_IN_THE_HOST=0` set. Please report what made you switch back (#2004).
- **Another purlis with a config home of its own has a session host of its own.** A second
  purlis on the same device that names its own config home, such as a development build beside
  the installed one, starts a session host of its own and runs its chats there (#2114).
