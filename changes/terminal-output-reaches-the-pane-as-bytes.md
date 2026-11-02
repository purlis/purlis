### Changed

- **A chat's output reaches its pane as the bytes the program wrote.** It used to be turned into
  text, escaped into JSON and read back on the way to the window; now the terminal decodes it
  itself, so output heavy with colours and cursor moves costs the window less to take in, and a
  character split between two pieces of output is put together by the terminal (#4745).
