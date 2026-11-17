### Changed

- **The project's `[policy]` template sets the sandbox level.** `open` runs no sandbox, `watch`
  and `hold` the basic sandbox, `team` the full one, and `strict` leaves it to `[sandbox]` as
  before; a committed `[sandbox] mode` overrides the template. A stricter level applies at once; a
  looser one, the team's sandbox off included, waits for your Accept (Q308 follow-up, Q328).
- **`network`, `ports` and folder and socket asks follow `[policy]`.** With `network` off or
  logged, a chat's proxy carries a host nothing lists instead of asking. `ports = "off"` lets a
  wrapped chat reach local sockets and ports of this Mac (purlis's own excepted); `ports` takes no
  `log`. With `asks` off or logged, a folder a chat may be granted is allowed for it and logged;
  a socket is allowed so only under `asks = "off"` (Q328).
