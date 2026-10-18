### Changed

- **A cluster operator pool's template and warm pool leave a namespace with its last Claim
  there**, and a Claim made at that moment is served all the same. Its ClusterRole gains
  `list` and `watch` on `sandboxtemplates` and `sandboxwarmpools`, which it watches by their
  metadata alone (#2260, docs/cluster-operator.md).
