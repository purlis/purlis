### Fixed

- **A keyring that did not start is still read as one that failed after a chat moves `.git`
  aside and back.** The note that a seal was kept for a plane now follows the plane's path, not
  its clone key, so an accepted `[policy]` stays enforced instead of dropping to the shipped
  default (#7164).
- **The window and a chat say that only opening purlis again mends a keyring that did not start**
  where a seal was kept, instead of asking the person to mend the list (#7164).
