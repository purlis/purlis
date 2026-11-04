### Added

- A persona can declare the credentials its work needs, one `secret-need.<KEY>: <purpose>` line
  each in its definition. An ask for a declared key shows that purpose. After you fill a key
  the persona does not declare, purlis offers adding it as a proposed change you apply or
  dismiss, so the next teammate's ask arrives named and explained. A first fill into a vault you
  don't have yet makes it in the system keyring; a persona with `vault: none` gets a vault named
  after it, and the proposed change sets it.
