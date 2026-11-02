### Added

- **A short task dispatched onto a cluster pool gets a runner of its own.** It starts from a warm
  runner where the pool keeps one. As the task closes, its branches come back to your clones under
  `refs/remotes/<pool>/fresh-…`, and the runner goes only once purlis has checked that your
  project holds every commit and change it holds. Where something is missing, the runner stays and
  the window says what, with Fetch again and Discard. Chats you work in, long tasks and chats on a
  subscription profile stay on the pool's runner that keeps their work.
