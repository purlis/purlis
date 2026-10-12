### Security

- **The project root's branch guard follows an alias after a program held in a variable.**
  `G=git; $G co -b x`, where `co` is an alias of `checkout` in the root's config or set on the
  line with `-c alias.co=checkout`, is now refused like `git co -b x`. The guard reads aliases
  there the way it already does after a substitution among git's options: one alias read per
  command line, and a line whose aliases git cannot be asked about is refused rather than
  allowed. A word that is no alias of a branch move, such as `$G status`, is still allowed
  (#1326).

### Changed

- **The update manifests say they are attested.** SECURITY.md now says that `dev.json`,
  `latest.json` and their weekly twins carry the same SLSA provenance as the builds, and how to
  check one with `gh attestation verify`. The app still checks each build's minisign signature
  before it installs anything (#778, #1133).
- **`persona remember --help` says where a persistent memory is written.** Only a unix build
  (macOS, Linux) writes one; elsewhere the write is refused, and `--ephemeral` scratch is
  written on every platform (#1408).
