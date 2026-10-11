### Fixed

- **A chat blocked on a local socket no longer sends you looking for an Allow.** A sandboxed
  chat that could not reach Docker's socket asked you to choose Allow for this chat, which a
  socket's notice never offers: purlis grants no local socket. A chat is now told that, and says
  which socket was blocked and that Start without the sandbox, or running the command yourself,
  is the way on.
