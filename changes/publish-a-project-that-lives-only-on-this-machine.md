### Added

- **A project that lives only on this machine says so, and publishes to GitHub or GitLab.** Its
  tab, and the Project level of its settings, read **This project lives only on this machine**
  when its folder is no repository yet or has no remote, with **Publish to GitHub or GitLab…**
  beside it. Publish asks what Create on a forge asks (the forge, the name, the owner, private
  unless you say public), then commits what is not committed yet, makes the repo with your
  purlis sign-in, scans the project's history for anything credential-shaped, pushes it through
  purlis's own credential helper and makes the repo the project's remote, which the push record
  holds for its saves. Each stage is shown as it runs. **Cancel**, or a stage that fails, takes
  back what the publish made: the repo, the remote it named, and the repository it started. A
  repo GitHub would not delete is kept, with a link to its settings, and **Retry** pushes into
  it (#1820). A project created on a forge now records its remote for its saves too.
