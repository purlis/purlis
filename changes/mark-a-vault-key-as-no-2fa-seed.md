### Added

- **Say a vault key's value is no 2FA seed.** A secret fill refuses a value that looks like a seed,
  and some real values look like one, such as a 16-letter app password. In a vault's tab, a key's
  menu has *Mark as no 2FA seed…*, and a marked key then fills as a plain value on the same notice
  and grants as any key. A chat can't make the mark, a new value takes it away, and an `otpauth`
  address is refused whatever the mark says (#5237).
