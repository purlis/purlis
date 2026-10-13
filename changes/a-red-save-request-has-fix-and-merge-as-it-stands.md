### Added

- **Fix in a chat, on a save's red pull request.** The red Notice in the Saving view starts a
  chat at once, at the project's root (for a workspace repo, in its workspace), on a message
  purlis writes: the pull request and its page, each failed check with its page, and what
  each one found. The chat fixes it and saves, and the save goes onto the same pull request.
  Pressing it again brings that chat forward. What a check says is quoted to the chat as
  data, and a link is given only to the forge the pull request is on.
- **Merge as it stands…**, on a red pull request and on one that waits with no end in sight
  (no check has started where the repository's other pull requests have them, purlis cannot
  read its checks, or a required check has not reported). It asks first, naming the pull
  request, the branch it goes into and the checks that failed, and merges without waiting for
  checks. A conflict, a required review and every other rule of the forge still stop it, and
  the refusal is shown with a link to the pull request.
- **A red pull request of a save is in the Inbox**, with the same two answers, until it
  merges or its checks pass.

### Changed

- **purlis's own look at a save's pull request uses your purlis sign-in.** A person signed in
  only through the window gets their green pull request merged by the look, with no new save.
  With no sign-in at all the pull request says a sign-in is needed, and offers **Sign in…**.
- **A pull request that waits with no end in sight says so**, in the Saving view and on the
  title bar's save mark.
