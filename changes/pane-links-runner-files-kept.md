### Changed

- **A runner chat's file link never opens the runner's own purlis folders or a harness's sign-in
  files.** That holds even after you answer the ask for a file outside the workspace. The link
  says why instead, and the runner refuses before it reads the file, whatever the path is
  written as: relative, with `..`, through a link, or in another case (#7064).
- **A hard link in a runner chat's workspace opens as the host opens one.** It is placed by the
  name the chat printed. One whose other name is in a folder the runner keeps does not open (#7077).
- **A runner chat's hover looks at a file outside its workspace as a hover on your machine
  does.** It never looks into a removable, network or automounted volume there, unless the
  chat's project or folder is on it (#7076).
