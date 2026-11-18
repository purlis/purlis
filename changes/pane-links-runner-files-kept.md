### Security

- **A runner chat's file link never opens the runner's own purlis folders or a harness's sign-in
  files.** That holds even after you answer the ask for a file outside the workspace. The link
  says why instead, and the runner refuses before it reads the file, whatever the path is
  written as: relative, with `..`, through a link, or in another case (#7064).
- **A hard link on a runner opens as the host opens one.** It is placed by the name the chat
  printed. One whose other name is in a folder the runner keeps does not open. Where the runner
  cannot tell, or its list of what it keeps cannot be read, the link says so and does not open
  (#7077).

### Changed

- **A runner chat's hover and tab look at a file outside its workspace as a hover on your machine
  does.** Neither looks into a removable, network or automounted volume there, unless the chat's
  project or folder is on it (#7076).
- **A runner chat's file reads less on its runner.** A hover reads only the file's first 8 KiB to
  tell text, and each piece of its tab only that piece (#7078).
- **A runner's file links need a runner of 1.94 or later.** An older runner shows no file links
  (#7064).
