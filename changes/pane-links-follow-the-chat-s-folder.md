### Changed

- **A relative path in a pane follows the chat's folder.** After a `cd`, `src/main.rs` links the
  file in the folder the chat is in now: a shell's pane learns it from OSC 7 at once, and a Claude
  Code chat from its next hook report. A file there outside the chat's workspace still asks first,
  as the same path typed in full does (#6367, ADR 0126).
