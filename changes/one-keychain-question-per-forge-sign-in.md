### Fixed

- **Starting the app asks the Keychain about a forge sign-in once, not several times.** Each of
  purlis's processes now reads a forge account's item once and keeps the token in its own memory
  for as long as it runs. A listing no longer asks again whether the item is there, and git's
  credential helper reads the token without asking that first. After an update of an ad-hoc
  signed build, macOS still asks once per process, until builds are signed with a Developer ID
  (#2658) (#2656).
