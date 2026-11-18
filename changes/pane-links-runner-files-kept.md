### Changed

- **A runner chat's file link never opens the runner's own purlis folders or a harness's sign-in
  files.** That holds even after you answer the ask for a file outside the workspace. The link
  says why instead, and the runner refuses before it reads the file, whatever the path is
  written as: relative, with `..`, through a link, or in another case (#7064).
