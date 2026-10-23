### Added

- **A send to a runner leaves out what a save would refuse, and says so** (#2046). What a
  pool's runner gets of a workspace's clones never includes what a save to the forge refuses to
  push: credential files, a file holding a token or a private key, a hard link. Changes names
  what the last send left out under each clone. A workspace's settings gain **Sent to runners**,
  where you can name more files and folders to keep on this machine.
