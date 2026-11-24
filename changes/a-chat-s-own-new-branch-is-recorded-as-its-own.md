### Fixed

- **A chat's own new branch is recorded as its own, in a plane too.** A project with no stored
  `[project] id` was recorded under no id, so a chat that made a branch through the forge route
  could not push to it again and was asked to be let in. The record and every read now use the id
  the session host holds the project by, so a chat's fast-forward pushes to a branch it made go
  through. Deleting a branch through the route is still refused.
