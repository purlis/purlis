### Changed

- **A run asks once for everything it needs.** A chat's `secret exec` that lacks keys, a
  program's Allow or a vault's raises one notice with all of them: a masked field for each
  missing key, and the program's Allow, or, for a declared tool on a vault its persona can't use,
  the vault's own Allow. What you type is stored even on Don't allow, and the chat hears "filled,
  but the program was not allowed". Allow once is used by the run it was for, so a run that had
  to wait doesn't ask again on retry (ADR 0112, #5242).
