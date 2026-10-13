### Changed

- **A vault's status says why its program is not run, not that it is "not on PATH".** purlis
  looks for `op` and `vault` well beyond `PATH`, so `purlis vault list`, `purlis persona list`
  and the vault's tab now say "op CLI not found", or "op CLI found only where a chat can write"
  where the only copy is one a chat could have written. A reference vault names each CLI under
  the same two reasons (#1517).
