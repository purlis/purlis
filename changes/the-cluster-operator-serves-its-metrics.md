### Added

- **The cluster operator serves its metrics.** Prometheus can scrape them at `/metrics` on the
  operator's port 8080, through the Service `purlis-cluster-operator-metrics`. They count claims
  by pool, namespace and state, warm runners by pool and namespace, and suspensions and claim
  deletions by pool and cause (#3737).
