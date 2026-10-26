### Security

- **On Linux, a sandboxed chat's socket listens only where it was bound.** The wrap's supervisor
  now makes a chat's `listen` itself, on the calling thread's own socket: an internet socket the
  chat never bound is refused, and any other socket listens as before. A refused listen is told
  as a blocked bind. A runner in a Docker pool is given the one call this needs, in a seccomp
  profile of its own: Docker's default and `pidfd_getfd` besides.
- **A Docker pool runner made before this version keeps working, but its chats' servers do not
  listen** (unix sockets included) until you remove the runner and add it again. A Kubernetes
  runner's chats listen nowhere yet (#3413).
