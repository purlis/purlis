### Added

- **One notice for every lane refused for unsigned commits.** However many lanes a forge
  refused, the Inbox lists them in one notice ("3 lanes can't merge: <repo> only accepts signed
  commits"), and the needs-you hand in the title bar counts it once. **Set up signing** opens the
  same dialog, which ends in **Sign and retry all 3**; with signing on, the notice offers that at
  once. A lane refused again stays listed with its reason; once all went, the notice says "3
  lanes signed and sent ✓" (#5633).
