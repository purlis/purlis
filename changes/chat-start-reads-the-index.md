### Changed

- A new chat's briefing reads the project index the session host keeps, so a chat starts quickly on a large project. The memory counts, the oldest todos and the other workspaces come from the index, and the unshared-memory line reads what the host last found, with no `git status` of its own. While the host builds a project's first index, the briefing says so and leaves out the other workspaces, the memory counts and the open todos.
