### Changed

- The build provenance of a dev build, of the runner image and of the cluster operator image
  now names the commit each was built from. Dev builds and the runner image are started by the
  push to `main` and built once that commit's checks pass, and the operator image is built by
  the operator's own checks of that push, so `gh attestation verify --source-digest <commit>`
  checks the commit. Before, a merge that landed while the checks ran made the provenance name
  that later commit.
