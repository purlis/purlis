### Security

- **`url` is updated to 2.5.9, which fixes a stack overflow in `Url::origin()` on nested `blob:`
  URLs** (RUSTSEC-2026-0592). It moves in the main workspace and in `tools/artifact-isolation-probe`,
  and clears the `cargo deny` advisories check.
