### Changed

- **The network record is written on time.** Repeats of a sandbox block held back within a minute
  are written when the chat's turn ends, when the chat ends and when the project is let go of,
  not at the next block the project hears; and a burst of connections is written a few seconds
  after its minute ends, not at the next connection (#1681, #1699).

- **Allowing a chat its own persona's grants is audited and recorded**, as every other Allow is
  (#1681).
