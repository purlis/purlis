### Changed

- **A test server on a port the system picks runs in a sandboxed chat on Linux.** A Codex or
  opencode chat on Linux may start a server on port 0 of 127.0.0.1, as Go's `httptest`, Node's
  test servers and Python's `HTTPServer(("127.0.0.1", 0))` do, with nothing to allow, and reach
  it there. It reaches no other port of this machine that way: only the ports its own servers
  hold. On every other address, and on macOS, the Notice that says what to do stays, and on
  Linux it now says to start the server on 127.0.0.1 (#3396).
