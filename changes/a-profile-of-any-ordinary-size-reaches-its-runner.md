### Fixed

- **A profile with skills reaches a pool's runner whatever its ordinary size** (#4081). Every chat
  on a pool was refused once a profile's settings passed 512 KiB, which a few skills do. A runner
  of this version takes them in several messages and keeps them all at once, or none, up to
  128 MiB and 8192 files. A skill's `node_modules`, `__pycache__` and `venv` folders stay on this
  machine. Past that the chat still starts, and its tab says which skills or files
  stayed behind and how to bring them.
