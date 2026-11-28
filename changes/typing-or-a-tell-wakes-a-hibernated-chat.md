### Added

- **Typing in a hibernated chat, or a parent's tell, wakes it.** What you type on its kept screen
  wakes the chat and reaches it once it is ready, after its wake line, once and in order; a
  `purlis dispatch tell` to a hibernated task wakes it to read the message, and `purlis dispatch
wait` on one answers at once that it is hibernated. A parent whose tasks have all reported, and a
  task nobody is waiting on, can now be hibernated by hand. A wake purlis cannot start leaves what
  you typed on the chat's band with the reason (SC-4j, #7639).
