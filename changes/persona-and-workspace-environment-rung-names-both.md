### Changed

- **`purlis persona list` and `current` name the variable that decided under both its names.**
  A persona or workspace chosen by the environment reads `via $PURLIS_PERSONA or
  $CHARTER_PERSONA` (and the same for the workspace), and the way out says to unset both, since
  the old name still decides when the new one is unset (#1344).
