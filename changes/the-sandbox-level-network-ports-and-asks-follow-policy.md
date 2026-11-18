### Changed

- **The project's `[policy]` template sets the sandbox level.** `open` runs no sandbox, `watch` and
  `hold` the basic sandbox, `team` the full one, and `strict` leaves it to `[sandbox]` as before; a
  committed `[sandbox] mode` overrides the template. A stricter level applies at once; a looser one,
  the team's sandbox off included, waits for your Accept (Q308 follow-up, Q328).
- **`network`, `ports` and folder and socket asks follow `[policy]`.** With `network` off or logged,
  a chat's proxy carries a host nothing lists instead of asking. `ports = "off"` lets a wrapped chat
  reach local sockets and ports of this Mac (purlis's own excepted, which a chat can neither move
  nor link to another name, under any `ports`); `ports` takes no `log`. With `asks` off or logged, a folder of the
  project a chat may be granted is allowed for it and logged, while a folder a later program loads
  code from still asks; a socket is allowed so only under `asks = "off"`. A host you kept blocked
  stays blocked in every mode (Q328).
- **The sandbox counts toward what a dead guard holds.** Where the sandbox is on, `watch` and `hold`
  included, a Bash guard that cannot start or answer now refuses the call as it would alive, where
  before it let the call run under `watch` with its secret checks logged. Under `open`, which runs
  no sandbox, it still lets the call run (ruling Q327).
