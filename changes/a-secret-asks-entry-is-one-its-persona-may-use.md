### Security

- **A secret is filled only with an entry its persona may use.** "Use an entry I have" on a Secret
  ask now lists only vaults the asking persona may use, and leaves out entries a browser login
  fills or a `mint` line marks, as a run's notice already did. A pick is checked again when you
  press, and a link is never made into a vault the persona may not use. A program you allow on a
  run's notice is never handed a key that links into a vault the persona may not use, whenever
  that link was made, so the notice can't hide where the value comes from (#5246).
