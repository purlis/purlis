### Changed

- **A host refused while a brokered command ran says the vault it ran with.** The chat's sandbox
  block Notice reads "while running a command with vault <name>", so you can judge whether that
  vault belongs at that host before you allow it. It names the vault, never a value, and says the
  chat's harness as a block of the chat's own command does. Such a host has a Notice of its own,
  apart from hosts the chat's own commands were refused (#1353).
- **A refusals list's Allow says the same words as every Notice.** Settings › Network and a
  chat's Network view now say _Allow for me on this machine_ and _Allow for everyone in this
  project_, the words the Notices and the Inbox use (#1700).
