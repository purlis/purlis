### Fixed

- **A project's view is no longer drawn again for an answer that says nothing new.** Each time
  the window read a project's save standing, its alerts or its stale clones, every open project
  was drawn again, even when the answer was the one already shown, and an alert moving in one
  project drew every project. Now only an answer that changes what a project shows draws it, and
  only the project in front reads its save standing every 10 seconds; one behind reads it when
  its files move, when the window is focused, after a save and as you come back to it.
