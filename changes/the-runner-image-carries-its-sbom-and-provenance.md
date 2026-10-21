### Security

- **The runner image carries build provenance and an SBOM.** Every runner image a release
  publishes has SLSA build provenance and a CycloneDX SBOM of each architecture's image (its
  packages and the `purlis` binary's crates), attached to the image in the registry and
  verified in the run that pushed it. docs/runner-image.md says how to check them (#2191).
