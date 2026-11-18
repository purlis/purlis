### Changed

- Pool setup's first step always offers three places: This machine, Another machine with Docker, and A Kubernetes cluster. The cluster card has a kubeconfig file field (your `~/.kube/config` when it exists) and a context picker for that file's contexts, so a cluster is reachable whatever Docker here is doing and whether or not a context was found. purlis keeps the file's path and the context's name on this machine, never in a project, and still never reads `KUBECONFIG` from the environment (#7191, #7192).

### Fixed

- Adding a cluster pool from Pool setup now uses the kubeconfig file chosen on the first step, and finds the credential plugin it signs in with in the usual install folders (Homebrew, `/usr/local/bin`, the gcloud SDK, `~/.krew/bin`) or the one folder you name. Only that folder is added to the commands purlis runs, never your shell's whole search path (#7192).

### Added

- A cluster pool's Options have its own "Longest wait for a cluster runner to be vouched for", in minutes (#7191).
