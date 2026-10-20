# Amended for #2123, every engine call ends within its limit, 2026-10-21

1. **A call's limit covers its writing.** A DevTools call's message is handed to the
   connection's writer, one thread that writes the calls in the order they were made, and the
   call waits for its answer only as long as its limit. An engine that stops reading fills its
   pipe and holds that writer, never a call: each call fails as not answered in time once its
   limit passes. At most 256 calls wait to be written; past that a call fails at once. A write
   that fails fails its call, and every later one, with why. *D-2123-1, decided in
   implementation, to be confirmed.*
2. **A stop ends within its grace.** A stop asks the engine to close and waits for its process
   within one grace in all, the close's wait included, then ends the engine's group.
   *D-2123-2, decided in implementation, to be confirmed.*
