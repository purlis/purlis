### Fixed

- **A GitLab merge request opened from the default branch through GraphQL is refused, as it is
  through REST.** A chat's `mergeRequestCreate` whose `sourceBranch` is the project's default
  branch is now refused when it is opened, inline or by variable, with the same reason as before:
  open it from your feature branch into the default branch.
