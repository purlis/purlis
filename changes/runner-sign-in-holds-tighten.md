### Security

- **A sandboxed chat keeps the folders above what it may not write in place.** A folder granted
  to a chat, and opencode's data folder, can no longer be moved aside to write under another name
  what the sandbox denies. Claude Code's sign-in on a runner opens only a page asking for Claude
  Code's own client and code page. A runner clears Claude Code's `hooks` folder from a kept
  profile, never clears a settings folder reached through a link, and clears what an earlier
  version left behind without holding up other profiles.
