### Security

- **A forge-route search is scoped only by a lower-case `repo:`.** GitHub reads `REPO:` or `Repo:`
  as text, so such a search ran over every repo the person's credential sees while the route
  treated it as a search of the chat's repo. Any other casing of `repo:`, `org:`, `user:`,
  `owner:`, `team:` or `enterprise:` is now refused, as are a qualifier with no name and a quoted
  value holding a `:`.
