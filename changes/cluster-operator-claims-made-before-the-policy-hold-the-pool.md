### Changed

- **A cluster operator Claim blocked by a pool's objects that belong to Claims made before the
  policy says so.** Its `Ready` reason is `HeldByClaimsMadeBeforePolicy`, naming those Claims;
  a Claim blocked by an object the operator never made still says `UpstreamObjectInTheWay`
  (#2580, docs/cluster-operator.md).
