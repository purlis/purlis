### Fixed

- **On macOS a brokered `secret exec` command can always read the credential file it was given.**
  macOS can name a file in the temporary folders by two paths. The command's sandbox now gives
  the file back under both of them, so the command is no longer refused its own credential now
  and then. It still reads nothing else in the vaults (#2489).
