### Security

- **A runner that answers with this desktop's own link key is not kept.** Pairing a runner now
  refuses a runner whose key is the one this desktop links with, as it refuses one with this
  desktop's device id (#2683).
