### Added

- **A sandboxed chat may run a server.** When a server a Codex or opencode chat starts is
  refused its port (`npm run dev` on 5173), its Notice offers **Allow only for this chat** or
  **Allow for me on this machine**. The server then runs on a socket of the chat's own, never on
  a TCP port of this Mac. purlis's front door answers on `http://127.0.0.1:<port>`, checks each
  request's address, and keeps your browser's cookies from the server.
  - The chat's browser opens the server with no second ask, and so can your own browser.
  - A page the server sends runs in your browser as a site of this Mac. It can read your other
    local apps' cookies that are not HttpOnly, send requests to those apps, and send what it
    reads to the internet. Open it in the chat's own browser unless you trust it.
  - `purlis listen <port> -- <command>` starts a server there, for example
    `purlis listen 5173 -- npm run dev`, and Vite, Next.js and other Node servers run unchanged.
  - A port below 1024, or one something on this Mac already listens on, is never offered.
  - The grant is listed in Settings' Network page with Remove.
  - A Claude Code chat's sandbox cannot be given one port to serve on, so its Notice says so and
    offers Start without the sandbox (#2019).
