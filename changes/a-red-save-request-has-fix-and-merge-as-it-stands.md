### Added

- **Fix in a chat, on a save's red pull request.** The red Notice in the Saving view starts a
  chat at once, at the project's root (for a workspace repo, in its workspace), on a message
  purlis writes: the pull request and its page, each failed check with its page, and what
  each one found. For the project, the chat fixes it and saves, and the save goes onto the
  same pull request. For a workspace repo, the chat fixes it and may commit in the clone; it
  does not push, and the fix reaches the pull request with the repo's next save, yours or
  the automatic one. Pressing it again brings that chat forward. If the checks pass or run
  again by then, nothing starts and the window says so. What a check says is quoted to the
  chat as data, and a link is given only into the pull request's own repository.
- **Merge as it stands…**, on a red pull request and on one that waits with no end in sight
  (no check has started where the repository's other pull requests have them, purlis cannot
  read its checks, or a required check has not reported). It looks at the pull request
  afresh and then asks, naming it, the branch it goes into and the checks that failed, and
  merges without waiting for checks. If the pull request changes between the question and
  your answer, nothing is merged and the window shows what is there. A conflict, a required review and every other rule of the forge still stop it, and
  the refusal is shown with a link to the pull request.
- **A red pull request of a save is in the Inbox**, with the same two answers, until it
  merges or its checks pass.

### Changed

- **purlis's own look at a save's pull request uses your purlis sign-in.** A person signed in
  only through the window gets their green pull request merged by the look, with no new save.
  With no sign-in at all the pull request says a sign-in is needed, and offers **Sign in…**.
- **A pull request that waits with no end in sight says so**, in the Saving view and on the
  title bar's save mark.
