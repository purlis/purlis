### Fixed

- **A folder the explorer could not watch is tried again on its own.** When the system would
  not watch an open folder (it ran out of watches, for example), the folder showed "Changes on
  disk are not shown yet" until you opened or closed another folder. purlis now tries it again
  every few seconds, less often the longer it is refused, and the note goes away as soon as it
  is watched. Closing a window that freed watches clears the note in the other windows at once
  (#1160).
