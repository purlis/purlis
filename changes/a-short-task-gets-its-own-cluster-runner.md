### Added

- **A short task dispatched onto a cluster pool gets a runner of its own.** It starts from a warm
  runner where the pool keeps one, and the runner goes as the task closes. Chats you work in, long
  tasks and chats on a subscription profile stay on the pool's runner that keeps their work.
