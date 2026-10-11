### Changed

- **The network record is written on time.** Repeats of a sandbox block held back within a minute
  are written when the chat's turn ends, when the chat ends and when the project is let go of,
  not at the next block the project hears; and a burst of connections is written a few seconds
  after its minute ends, not at the next connection (#1681, #1699).

- **Allowing a chat its own persona's grants is audited and recorded**, as every other Allow is
  (#1681).

- **A host refused before a chat had its number is still shown.** One purlis's proxy refused in
  the moment before the chat was numbered is raised as its Block once it is, not dropped (#1683).

- **The sentence about an older Claude Code counts as said once a tab shows it**, so a chat that
  never reached its tab does not use up the one telling (#1699).
