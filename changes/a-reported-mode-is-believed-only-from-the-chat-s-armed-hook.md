### Security

- **A chat's permission mode is taken only from the hook purlis armed for it.** The mode a chat
  reports, which decides whether its tasks start in auto, is now believed only from a report
  that carries a key purlis gives that chat's hook alone. Any other report's mode is read as
  the default, asking mode, so it can never raise the level a task starts at (#3822).
