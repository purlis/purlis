### Security

- **On Linux, a sandboxed chat's socket listens only where it was bound.** The wrap's supervisor
  now makes a chat's `listen` itself, on the calling thread's own socket: an internet socket the
  chat never bound is refused, and any other socket listens as before. A runner in a Docker pool
  is given the one call this needs.
