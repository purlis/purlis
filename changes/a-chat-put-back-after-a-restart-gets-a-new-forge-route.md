### Fixed

- **A chat that kept running while purlis restarted can push again.** Its own `git push` and `gh`
  were refused with HTTP 403, because the route they used went with the app. purlis now restarts
  such a chat on its conversation when its turn ends, with a new route, so it pushes its own
  branches again.
