### Added

- **A missing secret asks you instead of failing.** When a chat's `purlis secret exec` finds its
  key missing from the persona's vault, the Inbox asks you for it: type the value into a masked
  field and the waiting command runs with it, or choose Not now and the chat hears "declined".
  Only the window answers, and the chat never sees the value (#5068, ADR 0109).
- **A command waits on your answer only as long as its harness lets it.** It ends with "waiting
  on you for `<vault>/<key>`; retry once answered" before its harness would kill it, and the
  chat is told when you answer. Chats missing the same key share one ask that says how many
  wait, and a chat you declined isn't asked about that key again (#5071).
