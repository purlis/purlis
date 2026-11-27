### Security

- **A keyring that did not answer as purlis started no longer reads as no keyring** where this
  machine kept a seal before: nevers taken away are unknown, so no dispatch across personas
  counts, until purlis is opened again. Accept now says to open purlis again, not to press again.
  An acceptance of `[policy]` taken away while the seal key does not read is broken, never the
  shipped default (#7153).
- **A project policy that does not read denies a chat on no persona too**, and the chat is told
  purlis could not read the policy just now, not that only a committed change allows it (#7153).
- **A vault check in the window for a dispatched chat runs under its chain's policy**, as its
  brokered read does (#7153).
