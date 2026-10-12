### Added

- **Create a project on GitHub or GitLab, in one step.** The welcome's **Create a project** now
  asks where the project lives: **GitHub**, **GitLab** or **Only this Mac**. On a forge, give it
  a name, an owner (you, or one of your organisations or groups), private or public (private
  unless you say otherwise) and a stack template or none. **Create project** makes the repo with
  your purlis sign-in, lays out the project, commits it, pushes the first commit through
  purlis's own credential helper and opens the project, showing each stage as it runs. Without a
  purlis sign-in to that forge, the page asks you to sign in first, and the guided steps add their
  sign-in step. **Cancel**, or a stage that fails, takes back what this creation made: the repo on
  the forge and the folder, and only those (#1819). GitHub deletes a repo only for a token with
  the `delete_repo` scope, which purlis does not ask for; when it cannot delete the repo it made,
  purlis names it and the page where you delete it.
