### Removed

- **The `purlis-building-ui` skill is retired.** It only applied when developing purlis itself,
  yet it took room in every chat's skill list. No skill takes its place. Its guide is now
  `docs/building-ui.md` in the purlis repository, which the repository's `AGENTS.md` points to. A
  persona whose `skills:` list still names it gets a `purlis persona lint` warning that it was
  removed, `skill_load` says the same, and its past runs keep their name (#5725).
