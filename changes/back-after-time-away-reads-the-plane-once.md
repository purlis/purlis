### Fixed

- **Coming back to the window after time away no longer stalls it while it catches up.** While
  the window was hidden, every change to a project on disk had the window read that project
  again, so time away piled up hundreds of reads and redraws that ran before the window could
  draw again. Now the window reads each project once when it is shown again, after its first
  frame, however long it was away.
