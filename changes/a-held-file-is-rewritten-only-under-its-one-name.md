### Changed

- **A held file is rewritten only under the one name it has on disk.** When purlis writes a
  plain-file vault, a local vault registry or a key file, and that file has a second name (a
  hard link), purlis says so and leaves the file as it was, instead of giving the one name new
  bytes while the other keeps the old ones. Other project files are rewritten as before.
