### Added

- **Discard one hunk of an agent's change.** Each change in a file's *Show what changed* has a
  *Discard*, which puts it back as the branch's base has it and takes it out of the diff at once;
  a binary file's change goes whole. It warns when a chat is working in the folder, offers Reload
  or Overwrite when the file changed on disk, and is recorded in the chat's history as a Human edit
  of its file and lines, never its contents (#5406).
