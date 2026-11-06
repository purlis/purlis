### Added

- **`purlis doctor` lists the repos still at a workspace's root, and moves them into `repos/`
  when you ask.** Repos cloned before new clones went to `repos/` live beside `workspace.md` and keep working there with
  nothing done. The doctor's *repos outside repos/* row names them, and its fix, *Move into
  repos/* (`purlis doctor --fix move-into-repos`, or the row's Fix button), moves them one
  workspace at a time. It is all or nothing for each workspace: a workspace with a chat open in
  it is left as it is, and a failure partway puts everything back. Every branch folder cut from
  a moved repo is repaired, so it keeps working. Before it moves anything, the window names the
  chats started in a repo itself, which their harness can no longer resume; their session
  records still open. The doctor also warns about a folder beside the repo of its name in
  `repos/` (the one in `repos/` is the repo), and about an old repo named `repos`, which holds
  new repos back until you rename it.
