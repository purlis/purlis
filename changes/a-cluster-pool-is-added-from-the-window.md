### Added

- **Run a workspace's chats in your team's cluster.** The add pool dialog has a Kind: Cluster
  takes your kubeconfig's context and the name of a Pool of purlis's cluster operator, shows the
  cluster operator's key for you to check against the one your cluster's admin gives you, then
  claims a runner there as you, pairs it and keeps it. A workspace set to run on that pool runs
  its chats on that runner. Before every link purlis checks that the Pool still names the key you
  checked and that the operator still vouches for the runner it paired with; either changed is
  refused and asked about, as a Docker runner's changed key is. purlis runs your own `kubectl`,
  found where its installers put it. The pool keeps its own copy of the context, taken when it
  was added; to pick up a changed kubeconfig, add the pool again (#3292).
