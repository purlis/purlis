### Added

- **A Claude Code chat may run a server on macOS.** When a server a Claude Code chat starts is
  refused its port, its Notice offers **Allow only for this chat** or **Allow for me on this
  machine**, as a Codex or opencode chat's does. Start the server with
  `purlis listen 5173 -- npm run dev`: it is reached through purlis's front door on
  `http://127.0.0.1:5173`, with the same checks, and Claude Code's own sandbox settings stay as
  they were (#3316).
  - It works for Node servers such as Vite, Next.js and Express. Another kind of server cannot be
    given one port in a Claude Code chat: start it without the sandbox, or in a Codex or opencode
    chat.
  - Removing the grant in Settings' Network page stops the server being reached at once.
