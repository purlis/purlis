### Security

- **On Linux, a sandboxed chat's socket listens only where it was bound.** The wrap's supervisor
  now makes a chat's `listen` itself, on the chat's own socket: a socket the chat never bound is
  refused, and one it bound listens as before.
