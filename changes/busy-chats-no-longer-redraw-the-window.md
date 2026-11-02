### Fixed

- **A busy chat no longer redraws the whole window each time it moves.** Every move used to reach
  the window's title bar and switchers, in the project in front and in every project behind it.
  Now only what shows that chat is drawn again, and a list of asks that the move left unchanged
  changes nothing (#4727).
