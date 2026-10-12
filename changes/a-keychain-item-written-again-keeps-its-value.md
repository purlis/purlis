### Fixed

- **A keychain item written again keeps its value if the write is cut short.** purlis deletes
  an item and makes it again so that it is held to the app. A spare copy now holds the value
  until the new item is made, so a write that is refused or interrupted leaves the value in the
  keychain (#1309).
