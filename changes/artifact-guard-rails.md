### Security

- **A pinned artifact cannot carry a secret to the team.** Pinning an artifact, and every save
  of the project, refuses a folder holding something shaped like a secret, even one written as
  HTML character references (`&#x67;`) or percent-encoding (`%67`). A save reads every file of an
  artifact's folder, whatever its size, and the secret check everywhere else that reads through
  escapes now reads through these encodings too.

### Added

- **An artifact has a size cap of 8 MB.** A larger folder is not pinned, and one that grew past
  it shows both sizes in its tab, with Repin and Unpin, rather than a page cut short. A pinned
  artifact whose files are missing names where its page belongs and offers the same two.
