### Changed

- **Each new build of the app holds your keychain items again at its first launch.** Until
  builds are signed, the Keychain trusts the exact build that made an item, so the record of
  what the items are held to now names the build as well as the app. An item that would ask
  waits for your press, as it did after the rename (#1309).
