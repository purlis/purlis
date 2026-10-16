### Added

- **A Helm chart for purlis's cluster operator.** `operator/charts/purlis-cluster-operator/`
  installs the operator, its definitions (Pool and Claim, from the chart's `crds/`) and the
  policy on Claims, with the operator's image named by digest; the plain manifests in
  `operator/deploy/` are rendered from it and make the same objects. The chart is not
  published to a chart repository (#2237, docs/cluster-operator.md).
