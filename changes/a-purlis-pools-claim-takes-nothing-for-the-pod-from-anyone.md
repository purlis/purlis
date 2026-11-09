### Security

- **No writer adds to a purlis pool's runner pod through its upstream claim.** The cluster
  operator's `purlis-claims-use-their-pool` policy now refuses `env`, `lifecycle` and pod labels or
  annotations on a SandboxClaim of a purlis pool's warm pool, from any writer, not only from the
  operator. A claim already holding one is refused its next write; `docs/cluster-operator.md` has
  a query that lists them before you upgrade.
