### Fixed

- **Coming back to the window no longer redraws every open project.** Each time the window
  came back into focus, every project it kept drawn was drawn again, and each one asked git
  about every repo it holds to look for a stale branch or address. The window could stall for
  most of a second as those answers came in. Now a focus asks again only for the project in
  front, and only an answer that changes what it shows draws anything.
