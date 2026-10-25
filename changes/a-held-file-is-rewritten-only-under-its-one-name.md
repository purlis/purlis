### Changed

- **A held file is rewritten only under the one name it has on disk.** When purlis writes a
  plain-file vault, a local vault registry or a key file, and that file has a second name (a
  hard link), purlis says so and leaves the file as it was, instead of giving the one name new
  bytes while the other keeps the old ones. The refusal says what to do next: change the value
  at the file's source, or put a plain copy of the file in its place. When the other name is
  inside the project, the refusal names it. Other project files are rewritten as before.
