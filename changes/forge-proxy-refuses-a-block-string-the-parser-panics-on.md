### Fixed

- **A GraphQL block string with non-ASCII whitespace no longer ends a chat's forge request
  thread.** The parser the forge proxy reads a document with panicked on a whitespace-only line
  such as U+3000 between indented lines; the proxy now refuses such a document, and a panic in any
  request's handler ends that connection alone and gives its slot back (#7589).
