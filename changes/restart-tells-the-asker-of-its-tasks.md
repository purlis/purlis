### Fixed

- **A chat that dispatched tasks is told about them after a restart.** Before, a chat brought
  back at launch was told nothing about the tasks it had dispatched, and a report waiting for it
  sat there until the person typed. Now its first message after the restart lists each task by
  chat number and says where it stands: brought back and carrying on, still running, or ended
  because purlis could not bring it back. The message also names each task whose report waited
  for the chat, and the report reaches the chat as context (#5754).
- **`purlis dispatch wait` returns a report the app no longer holds in memory.** After a
  restart, or after an earlier turn had read the report, it used to say the app "no longer holds
  its text". It now reads the report from the task's dispatch record (#5754).
