### Fixed

- **A chat that names its conversation as it starts can always be restarted.** A Codex or opencode
  chat whose first report came while purlis was still starting it lost its conversation for good:
  every restart said it had none to resume, and the next launch could not resume it either (#5170).
