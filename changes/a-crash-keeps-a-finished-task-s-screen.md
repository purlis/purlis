### Fixed

- **A task that had finished when purlis crashed or was killed keeps its last screen**, so
  *Read it* on its card works after the next launch, even one that starts fresh. The launch reads
  the screen before it closes the task, under the same limits as a quit and only where Settings ›
  Project › Dispatch › Keep task screens is on. It waits at most 3 seconds for these screens,
  while the other chats start (#6504).
