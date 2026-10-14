### Added

- **The app starts its session host.** Opening purlis starts one session host for every open
  project, run by the app's own program so nothing new asks for the Keychain, and quitting ends
  it. It listens on a socket in a folder of your own under the runtime or temporary folder,
  admits the window, and refuses another user, a wrong credential and anything running inside a
  chat. No chat runs in it yet. `purlis serve` runs the same host on a machine with no window
  (#1997).
