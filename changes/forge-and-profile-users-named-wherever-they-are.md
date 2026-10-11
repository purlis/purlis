### Changed

- **A forge or a profile names every user before it goes.** Removing a forge in Settings now
  also names each workspace clone on its host that `inventory/repos.json` does not list, at its
  workspace. Removing or renaming a harness profile also names a persona whose `model:` line
  starts its chats on that profile. Changing a forge's kind or host in its row is now refused
  when another forge already holds that host as another kind, as adding one is (#1241, #1720).

### Fixed

- **Deleting a task's branch no longer blames git for purlis's own refusal.** When purlis cannot
  tidy away the folder's record first, the sentence now says so instead of "git would not
  delete" (#1720).
