### Fixed

- **A tool call recorded from a hook's spool keeps its argument digest.** When purlis was not
  listening, the spooled line used to drop the digest, so the call could not be matched with the
  same call heard live. The digest is now sealed to a key only purlis holds, so the chat cannot
  read it back, and it is opened and keyed as the spool is drained (#6880, #6676).

### Security

- **A sandboxed chat can no longer read the event log's keys.** The args key
  (`events/<device>/args.key`, which keys every argument digest) and the keys spooled digests are
  sealed to (`events/<device>/seal/`) are now denied to every chat, for reading and for writing,
  under every spelling of the data folder. Before this the sandbox did not deny them, so a chat
  could read the args key and match a digest it was never meant to read. A chat's seal key is now
  named for its project as well as its number, is deleted when the chat ends and its spool is
  drained, and is left out of backups.
