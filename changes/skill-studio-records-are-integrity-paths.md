### Changed

- **The Skill studio's records are held as integrity paths.** The records that say which
  workspace and persona folders a running chat could have made, the folders waiting for your
  Accept, the drafts, the tries and grades' arms, the grades of local copies, the grades under
  way, the grades' chats and the opt-in to grade after each edit are kept in the project's state
  folder. They are now held as purlis's own, like its other records there, so no sandboxed chat
  writes them, under either spelling of the state folder or where `$PURLIS_HOME` puts it.
