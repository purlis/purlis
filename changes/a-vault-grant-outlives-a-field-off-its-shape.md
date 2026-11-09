### Fixed

- **A vault grant on this machine is no longer dropped by an unrelated write.** When another
  field of this machine's sandbox file was off its shape, the next grant, revoke or chat count
  rewrote the file from defaults and took away the vault grants the window still showed. Every
  field that still reads is now kept, and only the one off its shape is left out.
