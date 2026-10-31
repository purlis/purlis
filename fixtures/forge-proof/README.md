# A worked proof for the operator's mint port

The runner (`crates/purlis-core/src/forge/mintport.rs`) and the operator (`operator/src/mint.rs`) are
built apart, so each test holds its side to these files. **Test values only: no key here is used
anywhere.**

- `link-key.hex`: a runner's private X25519 link key.
- `challenge.hex`: the public half of the operator's one-use key, as the port answers it.
- `shared.hex`: their X25519 agreement.
- `text.txt`: what is asked, as both sides spell it.
- `proof.hex`: HMAC-SHA-256 of `text.txt`, keyed by HKDF-SHA-256 of `shared.hex`, salt
  `purlis forge token request v1`, info `challenge.hex` as bytes, 32 bytes long.
