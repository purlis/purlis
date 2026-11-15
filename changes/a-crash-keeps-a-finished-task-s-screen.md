### Fixed

- **A task that had finished when purlis crashed keeps its last screen**, so *Read it* on its
  card works after the next launch. The launch reads the screen before it closes the task's
  session, under the same limits and the same Settings switch as a quit. It waits at most 3
  seconds for these screens, while the other chats start (#6504).
