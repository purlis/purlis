### Security

- **The update manifests carry build provenance, and the provenance is on the release page.**
  `latest.json`, `dev.json` and their weekly twins are now attested like every build file, so
  `gh attestation verify latest.json --repo purlis/purlis` checks them too. A job that holds
  no key and cannot write the release writes them. Each release also carries
  `purlis.intoto.jsonl`, the attestation's own bundle, for checking offline (#778, #1133).

- **A release build checks that the macOS app keeps the hardened runtime and carries no
  entitlement.** The check reads the signed app itself, on every macOS build, in both signing
  modes. A test also holds that no release build turns on the fence purlis's own tests run
  under (#1180, #1517).
