### Security

- **A forge your administrator pins as never allowed is refused for a chat entirely.** The pin
  now covers `gh` on the chat's route socket, the route's `CONNECT` door, the forge's API host
  (a pin on `github.com` covers `api.github.com`) and the git purlis runs for a chat, and it is
  read on every connection. A chat is never wired to a repo on a pinned forge, and the refusal
  says who set the pin and that signing in does not change it.

### Fixed

- **`purlis worktree push` and `update` reach a chat's forge through its route.** The git purlis
  runs for a chat now goes through a relay that carries the route's proof, where the door
  answered it `407`.
