# Amendment, 2026-09-30: the machine store is in the config home, not the application-data directory

"Where it lives" above says the store is under the OS application-data directory, through
Tauri's path API. **It is not.** `machine.rs` keeps it at `$PURLIS_CONFIG_HOME`, else
`$XDG_CONFIG_HOME`, else `~/.config`, then `charter/machine.json`: the ladder
`charter/report.py:consent_path` already used. The module note gives the reason. On macOS
the application-data directory is `~/Library/Application Support`, so following the platform
convention would have given a machine two `charter/` directories, one for report consent and
one for this store, holding the same kind of record. One address was judged worth more than the
convention. `layout.json`, `theme.json`, `extensions.json`, the plugin copy and the restart
marker sit beside it.

The Tauri application-data and log directories are still used, for the shell-tab shims and the
panic log. [ADR 0069](../0069-every-store-charter-writes-is-in-one-of-four-tiers.md) calls all of
these the **Machine** tier and says where each store in it is.
