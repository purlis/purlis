# A runner's link key statement, one worked example

The cluster operator (`operator/src/vouch.rs`) writes and signs this statement, and the desktop
(`crates/purlis-core/src/clusterpool.rs`) reads and verifies it. Both test against these files,
so the two never disagree on a byte (#2056).

- `statement.txt`: a statement, exactly as signed. Its runner namespace is the one both sides
  work out for its requester, `ada@example.com`: `purlis-runners-` and the first 16 hex digits of
  the SHA-256 of the requester (#2055). Its cluster is `cluster.txt` (#3305).
- `cluster.txt`: the cluster it was made in, as a Pool's status names it: the uid of that
  cluster's `kube-system` namespace.
- `test-only-signing-key.pkcs8.hex`: an Ed25519 key as PKCS#8 DER, in hex, whose seed is the
  bytes 1 to 32. It is a test vector, known to everyone, and signs nothing real.
- `operator-key.txt`: its public half, spelled as a Pool's status spells it.
- `signature.txt`: its Ed25519 signature over `statement.txt`.

The same operator, rolling its key (#3305):

- `test-only-next-signing-key.pkcs8.hex`: the next key, whose seed is the bytes 33 to 64. A test
  vector too.
- `next-operator-key.txt`: its public half, as a Pool's `status.nextSigningKey` spells it.
- `next-signature.txt`: its signature over `statement.txt`, as a Claim's
  `status.linkKey.nextSignature` carries it.
- `key-roll.txt`: the current key's word for the next one, exactly as signed, as a Pool's
  `status.nextSigningKeyVouch.statement` carries it.
- `key-roll-signature.txt`: the current key's signature over `key-roll.txt`.
