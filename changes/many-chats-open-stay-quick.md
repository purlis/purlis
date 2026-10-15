### Fixed

- **Opening a chat stays quick with many chats open** (#2157). Each time a chat opened or
  closed, purlis compiled every open chat's sandbox again to see whether any ran on an older
  one; now chats started alike are compiled once for that check, so opening the fiftieth chat
  costs about what opening the first does.
