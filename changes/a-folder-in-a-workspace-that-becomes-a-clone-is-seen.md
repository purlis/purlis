### Fixed

- **A folder in a workspace that becomes a clone shows up without a restart.** Running
  `git init` in a plain folder of a workspace, or moving a `.git` into it, used to change nothing
  the window heard, so the workspace went on showing the folder as it was until the project was
  read again. purlis now watches each plain folder a workspace holds (the folder itself, not
  what is inside it), and reads the workspace again when one becomes a clone (#1160).
