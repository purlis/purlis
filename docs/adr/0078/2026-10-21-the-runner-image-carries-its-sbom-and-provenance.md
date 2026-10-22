# The runner image carries its SBOM and build provenance, 2026-10-21

purlis's runner image is signed keyless with cosign (#2043). The app's release files also carry
SLSA build provenance (#583) and a CycloneDX SBOM (#584); the image now does as well (#2191).
Items 1 to 4 are **decided in implementation, to be confirmed** (D-2191-1 to D-2191-4).

1. **Where they live.** Each is a GitHub attestation, signed through the run's OIDC identity and
   pushed to the registry beside the image (`actions/attest`, `push-to-registry`), in the same
   private package. Provenance is on the multi-architecture index, the digest a release names
   as `runnerImage`. Each architecture's SBOM is on that architecture's image, the one a pull
   resolves to. Rejected: buildx's own `provenance`/`sbom` attestations. They change the
   pushed manifest into an index of their own and carry no Sigstore signature that
   `gh attestation verify` checks. *D-2191-1.*
2. **What the SBOM lists.** syft, at the version and checksum `release.yml` pins, reads each
   pushed image by digest. The SBOM lists the image's Ubuntu packages and, read out of the
   `purlis` binary that `cargo auditable` built, the crates it was compiled from. An SBOM that
   names neither `pkg:cargo/purlis-cli` nor a `pkg:deb/` package fails the run. *D-2191-2.*
3. **Who may attest.** `sbom` holds `contents: read` and `packages: read` and nothing else. One
   job, `attest`, may store an attestation; it checks nothing out and runs nothing of the
   commit's. `release.yml`'s call grants `attestations: write` because the called workflow's
   jobs need it. *D-2191-3.*
4. **Verified in the same run.** `attest` verifies the provenance and each SBOM with
   `gh attestation verify`, pinned to `runner-image.yml` in this repository at this run's own
   workflow commit, this commit and a GitHub-hosted runner, before the run can succeed. A release's call fails when they do not
   verify, so no release names an image that lacks them. Each job that names an image by an
   architecture's digest first checks the digest file holds a sha256 digest and nothing else. `sbom` reads the images beside
   `index`, so the release waits on `attest` alone (ten minutes at most). *D-2191-4.*
