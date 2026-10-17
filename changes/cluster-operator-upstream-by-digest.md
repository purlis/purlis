### Security

- **The cluster operator's pinned upstream agent-sandbox controller is installed by digest.**
  Step 1 of the install is now
  `kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6`, which applies the
  vendored release with its controller named by the release's digest; doctor reports the
  release by its tag alone as missing (#2261, docs/cluster-operator.md).
