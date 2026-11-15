### Changed

- **A chat never merges a pull or merge request; you do, from Asks.** A chat that runs `gh pr
merge`, `glab mr merge` or any other merge purlis recognises is refused it, attended or not,
  and nothing is merged. Where purlis can tell which request it is, and the repo is one the
  project lists on that forge, purlis reads the request from the forge and Asks shows one merge
  ask for it, before the chat is told you were asked. The ask is high-risk: it is never grouped,
  keyboard triage passes it by, and Merge asks once more, naming the
  commit it merges. Merge there is purlis's own call to the
  forge, made with your sign-in, at the commit and into the branch the request had when it was
  asked, by the repo's own merge method, and through its merge queue or train where it has one.
  If the branch has moved since, the forge refuses; if the request now merges into another
  branch, purlis refuses; either way nothing is merged and the ask stays, and when the chat tries
  again at a new commit, the ask at the old one is gone and a new ask, naming that commit, takes
  its place: nothing you pressed on the old one carries over. A forge that sets the request to merge later instead is
  an error you are shown. Where purlis raises no ask (the repo is not the project's, the forge
  could not be read, the chat has three merges waiting already, or the app did not answer), the
  chat is told nobody was asked, and the refusal is recorded. Don't merge merges nothing. A merge
  is never remembered as a rule (#6402).
- **In a chat purlis started, `purlis change land` is refused,** as is a git or forge CLI alias
  that could publish or merge (`git config alias.x push`, `gh alias set` of `api`, `release`,
  `auth` or `config`): landing is yours to do from the window now. The chat is told so, and that
  `gh pr merge <number>` asks you to merge one request in Asks. Your own terminal is unchanged
  (#6402).
