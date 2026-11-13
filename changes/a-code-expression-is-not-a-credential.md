### Fixed

- **A script or a patch that reads a credential from somewhere is no longer refused as one.** The
  save guard, a written memory's warning and a dispatched brief's check read an assignment to a
  credential's name (`token`, `secret`, `password`, `api_key`) as a secret whenever its value was
  code: a call, a member path, a lower- or camel-case identifier, a shell expansion. Such a value
  now passes, in Rust, Python, JavaScript, TypeScript and shell alike. A quoted literal, a token
  by its issuer's shape and a long literal are still refused, inside code or beside it (#6292).
