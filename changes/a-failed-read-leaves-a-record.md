### Fixed

- **A file read the sandbox refused is now on the record.** A failed `Read` was heard on its success
  line only, so a refusal left no block and no Notice; it is read as a failed command is (#6888).
