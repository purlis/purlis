### Fixed

- **Keep blocked on a vault the persona can already use says so.** If the persona was given the
  vault first, by Always allow or the refused vault's Allow, or the vault is tagged for it, the
  chat's runs on it still go. The Notice now says that and where to stop them (the Allowed list,
  the vault's tag or the persona's tool line), and the chat is told not to use the vault unless
  you say so, instead of "kept blocked, don't retry" (#5801).
