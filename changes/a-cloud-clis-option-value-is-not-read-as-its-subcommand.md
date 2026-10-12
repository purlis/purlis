### Fixed

- **A cloud CLI's option value is no longer read as its subcommand.** An ask for
  `aws --region eu-west-1 s3 rm …` read as an ordinary command, because the region was taken
  for the subcommand. The global options that take a value are now skipped for `aws`, `gcloud`,
  `az` and `kubectl`, so such a line is classed as the delete or the secret read it is. The old
  reading is kept beside it, so no line is classed as less risky than before. `az` deletes,
  purges and Key Vault secret reads are classed too (#1046).
