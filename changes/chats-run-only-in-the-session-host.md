### Removed

- **Chats run only in the session host.** The way back to running chats in purlis's own window,
  `PURLIS_CHATS_IN_THE_HOST=0`, is gone: a launch that still has it set says in its log that it
  is no longer read, and runs its chats in the session host as every launch has since 0.5.0. To
  clear it on macOS, run `launchctl unsetenv PURLIS_CHATS_IN_THE_HOST` and start purlis again
  (#2005).
