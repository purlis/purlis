### Fixed

- **A harness declaration's `chosen_by = "purlis"` loads.** The reader took only `"charter"`,
  so a declaration written as the project format documents it was refused. `"purlis"` is the
  word, and `"charter"` is still read as its old spelling (#1756).

### Security

- **The approval dialog for a declared harness shows the newline it types.** A declaration's
  `[terminal] newline` is typed into the program's terminal, so the dialog now shows its bytes
  with the program and its words, and you approve them together (#1119).
