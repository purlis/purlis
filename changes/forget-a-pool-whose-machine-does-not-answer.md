### Fixed

- **A pool on another machine that doesn't answer can be forgotten.** Remove used to fail with
  no way out when the pool's machine was gone or off this network. Its row now offers *Forget it
  here*, which forgets the pool on this machine without contacting the other one, and says
  what stays there and the `docker` commands that remove it.
