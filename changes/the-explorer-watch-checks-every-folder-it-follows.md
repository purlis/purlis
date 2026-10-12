### Fixed

- **The explorer's watch holds every folder to the same check, each time.** A folder an open
  window had expanded was checked when the window asked for it, and its timed retry checked it
  again, but a later watch started by another window did not. Each watch now checks every folder
  it follows, and a folder that fails the check is no longer watched: its window shows that its
  changes are not shown. A folder refused while the retry waited out a long pause is now tried
  again within a few seconds, not after up to a minute (#1160).
