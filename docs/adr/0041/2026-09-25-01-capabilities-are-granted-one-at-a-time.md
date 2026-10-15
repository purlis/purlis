# Amended 2026-09-25: capabilities are granted one at a time

[ADR 0053](../0053-an-extension-is-granted-capabilities-one-at-a-time.md) adds a `capabilities` list to
the manifest. A word this purlis does not know refuses the whole manifest, by name, and every
capability is named in the approval prompt and covered by the fingerprint. Each capability is
added in its own change, with its own amendment here. Where this record's two amendments of
2026-09-22 and 2026-09-23 disagree about which gate items are met, the 2026-09-23 one supersedes
the earlier, and its item-by-item account of the gate is the current one. Nothing here changes decision 2's
table. No capability can reach the machine store, `reopen.json`, harness profiles or vaults.
