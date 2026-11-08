### Removed

- **The `purlis-building-ui` skill no longer ships with purlis.** It only applies when developing
  purlis itself, so it took room in every chat's skill list. Its guide is now
  `docs/building-ui.md` in the purlis repository, which the repository's `AGENTS.md` points to. A
  persona's `skills:` list that names it now reaches a project skill named `building-purlis-ui`
  where the project has one, and `purlis persona lint` says to write that name (#5725).
