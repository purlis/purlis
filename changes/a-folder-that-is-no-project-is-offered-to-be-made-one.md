### Changed

- **Opening a folder that is not a purlis project offers to make it one.** Picking such a folder
  through **Open a folder on this machine…** or typing its path no longer ends in a refusal: the
  welcome says "This folder isn't a purlis project yet. Make it one?" with the project template
  choice. **Make it a project** opens the folder in a workspace of its own in the project purlis
  keeps on this machine, through the usual trust question; nothing is written into the folder,
  so its code stays where it is. **Not now** leaves the folder untouched (#1822).
