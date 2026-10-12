### Fixed

- **A hook call the app was slow to take is recorded within seconds, and only once.** A chat's
  hook keeps a line in the chat's spool when the app does not take it within a quarter of a
  second. That spool used to wait for the chat to close or the project to open again, and a
  line the app took late was recorded twice, once as it came and once from the spool. The app
  now drains a running chat's spool a couple of seconds after it next hears from the chat, and
  a line it already recorded as it came is not recorded again from the spool (#952).
