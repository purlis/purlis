### Added

- **A team rolls its cluster operator's signing key without any desktop asking again.** Add
  the next key to the operator's Secret as `next.pem`: every Pool names it as
  `nextSigningKey`, with the current key's word for it (`nextSigningKeyVouch`), every Claim's
  statement is signed by both keys, and a desktop that sees the roll moves its pin. Then make
  `next.pem` the `key.pem`. The operator's pod mounts the whole Secret, so no restart is
  needed (#3305, docs/cluster-operator.md).

### Security

- **A cluster runner's statement names the cluster it was made in.** Every Pool names its
  cluster as `status.cluster`, the uid of its `kube-system` namespace; a desktop pins it with
  the operator's key and refuses a statement of any other cluster. The statement is now v3, so
  a desktop and an operator of this release go together (#3305).
