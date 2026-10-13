### Added

- **Update from remote, in the window.** Where the Saving view and the title bar say how many
  commits came in (`↓N`), there is now an **Update** to press. purlis brings the commits in
  itself, outside every chat's sandbox, from the remote recorded for the project and with your
  purlis sign-in, held as every fetch is. A project that is only behind is fast-forwarded; one
  with commits of its own has them replayed on top. A project with unsaved work is never
  touched and nothing is stashed: the button is *Save, then update* where a save can run. In
  the request modes (`pr`, `pr-merge`), commits an open request carries wait for it to merge,
  as the mode says; commits no request carries yet are replayed.
- **An update leaves the project at the new commit, or exactly as it was.** Nothing starts
  where a file the incoming commits change cannot be written or a file is in the way, and an
  update git stops part-way is undone and checked. Why it was left is a Notice in the Saving
  view, with the files and a way out.
- **A chat refused an update of the project's root asks you for it.** The refusal now names
  the window's Update, and the chat gets a needs-you item that says so.

### Fixed

- **A save's rebase that failed is called a conflict only when it is one.** A file git could
  not write, a full disk or a git that died used to be reported as a conflict, and `git rebase
  --abort` ran where no rebase had started, which could leave the project half-updated. Such a
  failure is now said in git's words, and what the failed checkout wrote is put back.
