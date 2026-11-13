### Changed

- **A chat never merges a pull or merge request; you do, from Asks.** A chat that runs `gh pr
  merge`, `glab mr merge` or any other merge purlis recognises is refused it, attended or not,
  and nothing is merged. Where purlis can tell which request it is, and the repo is one of the
  project's, Asks shows one merge ask for it. Merge there is purlis's own call to the forge,
  made with your sign-in, at the commit the request was at when it was asked: if the branch has
  moved since, the forge refuses and the ask stays. Don't merge merges nothing. A merge is never
  remembered as a rule (#6402).
