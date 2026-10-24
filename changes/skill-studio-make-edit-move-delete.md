### Added

- **Make, edit, rename, move and delete skills in the Skill studio preview** (#2770).
  - **New skill** starts one from a template, in the workspace you opened the studio from, or at the project's root.
  - **Edit** opens a skill in the light editor. A live lint says what would stop it from loading, and Save stays off until the skill is whole.
  - **Rename** also renames every persona's and skill's mention of it.
  - **Moving** a skill is done by dragging its row onto a level, or with Move to. Moving one into the project's skills asks where it reaches.
  - **Delete** says who uses the skill first, and Restore brings it back with its grant. Where the grant can't go with it, the skill stays where it was and you're told why.
  - **A project skill's grant**, and a workspace's switch to turn it off, are on its row. A workspace can't turn on a skill it wasn't granted, and is told which grant would reach it.
  - **Each change is one commit** of only that skill's files.
  - The same changes are on the command line: `purlis skill new`, `edit`, `mv`, `rm` and `restore`.
- **A running chat whose skills changed offers a restart** on the same conversation, so it picks up the new ones.

### Changed

- `purlis skill grant` and Share now commit their change right away, as one commit, instead of leaving it for the next save.
- A chat can't run `purlis skill` to change a skill. Changes made from a chat are coming as drafts you accept in the studio.
