### Fixed

- **Allow once on a secret fill is no longer asked again when the fill's retry waits on its
  key.** If the vault lacked the key and the chat's call ended before you added it, the retry
  asked for your Allow a second time. Your Allow now carries that retry of exactly the same fill
  in the same chat, and still fills only once.
