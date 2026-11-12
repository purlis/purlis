### Added

- **Pool setup sets a pool up on your team's cluster too.** Add a cluster pool… now opens Pool
  setup on the cluster path. Where recommends a context your kubeconfig names, and keeps only its
  name. Check the cluster shows doctor's findings before anything changes, and a cluster that
  lacks something stops there and says what to do. A cluster where purlis's cluster operator
  already runs passes straight through. `purlis pool setup` shows the same lines (#4492).
- **Pool setup installs purlis's cluster operator, a part at a time.** Install has agent-sandbox,
  purlis's definitions, the signing key and the operator, each on its own button that says what
  it applies, each run again on its own, with your own sign-in. A signing key that is there is
  never replaced, and a new one is made in memory and handed to the cluster alone. Uninstalling
  is never a button: Install shows the commands, and how many people and claims they would end
  (#4493).
