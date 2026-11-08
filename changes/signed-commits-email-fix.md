### Added

- **Set up signing fixes a commit email the forge won't verify.** GitHub and GitLab show a
  signed commit as Verified only when its email is one of the account's verified addresses. When
  it isn't, the Check step says so in a sentence and offers those addresses (on GitHub, the
  private noreply one too) to sign chat commits as, then checks again. With none verified, it
  links to the forge's email settings (#5631).
