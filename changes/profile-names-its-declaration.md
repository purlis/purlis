### Added

- **`purlis profile create --declaration anthropic` makes a Claude Code profile that signs in
  with a key from your keys.** It is made with no key, and `purlis profile list` shows the
  declaration it names. Until chats on a key can start in this build, a start on a key profile
  is refused in words, and so is a start on a profile whose key is still in a vault (T1a-12,
  #6757).
