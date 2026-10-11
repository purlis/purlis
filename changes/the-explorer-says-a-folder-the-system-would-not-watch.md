### Fixed

- **The explorer says when the system would not watch a folder.** When the system refused to
  watch one open folder and watched the rest, that folder looked live but never showed what
  changed on disk. The tree now says so on that folder, in every window that has it open, and
  tries again when you close and open it (#1160).

- **A branch's ahead and behind counts do less work.** Each side is counted from where the two
  branches meet, so a base far ahead of the branch is no longer walked again for the ahead count
  (#1152).
