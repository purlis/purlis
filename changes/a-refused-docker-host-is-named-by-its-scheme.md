### Security

- **A refused Docker engine is named by its scheme alone.** When a pool's `DOCKER_HOST` points
  at an engine purlis cannot tell is on this machine, the refusal says "a tcp:// address", never
  the address itself, which may hold a user and a password.
