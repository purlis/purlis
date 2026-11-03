### Added

- **The cluster operator's image carries build provenance and an SBOM.** As the runner image
  does, each published operator image now has SLSA build provenance on its index and a
  CycloneDX SBOM of each architecture's image, both verified before `main` moves. See
  `docs/cluster-operator.md` for the `gh attestation verify` commands.
