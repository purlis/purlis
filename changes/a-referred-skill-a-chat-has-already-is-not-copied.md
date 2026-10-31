### Fixed

- **A skill another refers to is no longer copied where its chats have it already** (Skill
  studio preview, #4154). Installing into a workspace now leaves out a skill it refers to when a
  project skill of that name is granted to the workspace and not turned off there, and installing
  for a persona leaves out one its `skills:` list names. Before, a copy landed beside it and hid
  the project's. A local install also relies on the level's shared skills. A shared install still
  brings its own copy where only a local skill has the name, since a teammate does not have that
  one.
