### Fixed

- **The explorer says when a branch's folders are not watched, and watches them again.** When
  purlis's reader had paused a branch (its reads kept running past their time or memory) or had
  no answer for it, the folders you opened in it were left unwatched with nothing said, so the
  tree looked live but never moved. The tree now says "Changes on disk are not shown yet" on the
  branch's topmost open folder, asks for the branch again once the pause is over, and reads the
  folders again once they are watched (#1727).

- **"Start a chat here" no longer holds the window while the reader is busy.** Finding the
  branch's folder could wait on a busy reader for two of its deadlines, on the thread that draws
  the window. It now waits off it (#1727).

- **The branch cockpit follows a checkout in its folder after a failed look-up.** A look-up of
  the cockpit's refs that failed unexpectedly left its folder marked as being looked up, so
  later checkouts there were not followed (#1152).
