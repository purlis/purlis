### Fixed

- **A token given while purlis moves its keychain items to the app leaves no old item behind.**
  Storing or replacing a vault's token, and the move of its items to the app's own keychain
  access after the rename, now take turns on the vault's keys. A token replaced during the move
  no longer had its old item written back with nothing referring to it (#1180).
