### Changed

- **Every vault command in a sandboxed chat says what works.** `secret set`, `secret rm`,
  `secret audit`, `vault add`, `vault remove` and `vault verify` are yours: in a sandboxed chat
  each is refused before anything is read, with a sentence naming the vault's tab in the app or
  the same command in your own terminal, rather than ending in the sandbox's own refusal
  (#1969).
