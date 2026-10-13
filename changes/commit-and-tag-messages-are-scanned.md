### Security

- **A chat's commit message is read for credentials when it commits.** A chat's own commit, and
  a commit the app makes for a sandboxed chat, is refused when its message holds what looks like
  a credential, masked, with the same needs-you item as a refused change. A push now also reads
  every header line of each commit and annotated tag it sends, and reads messages in UTF-8
  whatever the repository's settings ask git to write them in (#1133).
