### Fixed

- **A tool call recorded from a hook's spool keeps its argument digest.** When purlis was not
  listening, the spooled line used to drop the digest, so the call could not be matched with the
  same call heard live. The digest is now sealed to a key only purlis holds, so the chat cannot
  read it back, and it is opened and keyed as the spool is drained (#6880, #6676).
