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
- **Pool setup goes on to a working chat on a cluster pool.** The Pool step makes a Pool in your
  cluster where it has none, then shows the cluster operator key it names and claims your runner
  only once you press again. A teammate on a cluster that has pools is offered them, and passes
  Check the cluster and Install. Sign-ins asks you to mark the pool as your own first, Agent
  identity shows the commands that let the cluster mint the agent's tokens, with a key of the
  App's own, and Try it starts a real chat there (#4494).
