### Fixed

- **A merge of a merge pushes without being taken for an allowlist change.** The pre-push scan
  compared a merge's allowlist with the parents history simplification left it, so a merge one
  of whose sides never touched the allowlist was refused as writing its own; it now compares
  against the merge's real parents (#6035).
