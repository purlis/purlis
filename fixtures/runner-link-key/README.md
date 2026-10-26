# A runner's link key statement, one worked example

The cluster operator (`operator/src/vouch.rs`) writes and signs this statement, and the desktop
(`crates/purlis-core/src/clusterpool.rs`) reads and verifies it. Both test against these files,
so the two never disagree on a byte (#2056).

- `statement.txt`: a statement, exactly as signed.
- `test-only-signing-key.pkcs8.hex`: an Ed25519 key as PKCS#8 DER, in hex, whose seed is the
  bytes 1 to 32. It is a test vector, known to everyone, and signs nothing real.
- `operator-key.txt`: its public half, spelled as a Pool's status spells it.
- `signature.txt`: its Ed25519 signature over `statement.txt`.
