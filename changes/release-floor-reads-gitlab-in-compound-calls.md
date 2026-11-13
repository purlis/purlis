### Fixed

- **A run with nobody watching can read GitLab in one compound call.** The release floor no
  longer refuses a `glab api` or `gh api` read because it carries a repository flag (`-R`,
  `--repo`) or a flag the floor does not know, once its method is spelled out (`-X GET`), when
  its endpoint is built from a variable such as a loop's `$id` (#6294). A call the floor still
  cannot read now says which part, the endpoint, the method or the flag, and how to split the
  call so it can be read.

### Security

- **A release or a tag made through the API is on the release floor.** Unattended, a write to
  a forge's releases, to `repository/tags` or `git/tags`, or to a `refs/tags/` ref is refused,
  as `glab release create` and `git tag` already were (#6294).
