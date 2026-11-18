### Changed

- Pool setup's first step always offers three places: This machine, Another machine with Docker, and A Kubernetes cluster. The cluster card has a kubeconfig file field (your `~/.kube/config` when it exists) and a context picker for that file's contexts, so a cluster is reachable whatever Docker here is doing and whether or not a context was found. purlis keeps the file's path and the context's name on this machine, never in a project, and still never reads `KUBECONFIG` from the environment (#7191, #7192).
