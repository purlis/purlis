### Fixed

- **A server that did not start is said as one.** A sandboxed chat's server refused with no port
  you could allow was shown as a block of a system folder. It is now shown as a server the chat
  started, which did not start (#3440).
- **A test server on a port the system picks gets a Notice.** A test suite that starts its
  servers on port 0 (Go's `httptest`, Node bound to port 0, or any program whose refusal the
  sandbox reports, Python's among them in a Claude Code chat) was refused with no Notice at all. Its Notice now says there is no one port to allow, how to start the server on a port of
  its own (`purlis listen 3000 -- <command>`), and offers Start without the sandbox for tests
  that pick their own ports (#3396).
