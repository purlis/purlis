### Changed

- **`purlis doctor` checks your vaults and the files that hold credentials.** The `vaults` row
  says, for each vault, whether its credentials are where it says they are: a keyring vault's
  items are asked for by their attributes only, a plain-file vault's file is looked for, and a
  1Password or reference vault's program is looked up. The `credential paths` row warns of a
  plain-text vault file that other accounts can read or that is reached through a link. Neither
  row reads a value or runs a provider's program (#994).
