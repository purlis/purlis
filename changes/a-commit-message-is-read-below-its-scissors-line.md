### Security

- **A commit message is read whole, as git may store it.** A chat's commit is refused for a
  credential anywhere git keeps in its message, and a `git commit -v` still commits. The
  project-root guard also keeps what a line makes after a `cd` it cannot follow as a directory
  it cannot name.
