### Added

- **A held merge's row says whether you have looked at its head, and who pushed it.** View
  changes, on the row in Asks and on the chat's tab, shows the request's changes in purlis and
  marks its head as viewed. The row says so, or that someone other than its author, an app or a
  bot approved this head on GitHub, or how many commits came since you looked. A head the chat
  didn't push, by the pushes purlis made for it, is flagged. While a merge waits, purlis watches
  the request, and a head that moves redraws its row, which asks again. A merge still takes two
  presses, whatever the row says. What was viewed is kept only while the app runs (#6432).

### Changed

- **Merge merges only what its row showed.** A press now hands back every fact its row showed,
  so checks, a title or an approval that changed after Merge asked once more ask again, as a
  changed ruleset already did (#6432).
