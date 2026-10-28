### Fixed

- **A chat put back from the session host keeps the skill routing it started with** (Skill
  studio preview, #3578). A chat whose workspace or persona chose a router extension is no longer
  routed again when purlis reopens and finds it still running: its router is not asked a second
  time, and Use skill… and the Skill studio say what that router hid at the chat's start, which is
  the list the chat actually holds. A chat started again is routed anew.
