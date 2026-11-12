### Changed

- **`purlis signing check` says where it waits for you.** While it waits for your yes in the
  window, it now prints "waiting for the person: Asks (✋) in the purlis window, for up to 2
  minutes", as the pool sign-in and take-off confirmations already did. All three now print it
  once the ask has reached the window, so a command that finds no window doesn't claim to wait.
