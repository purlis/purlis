### Added

- **A held merge's row says whether you have looked at its head, and who pushed it.** View
  changes, on the row in Asks and on the chat's tab, shows the request's changes as GitHub gives
  them to purlis's own sign-in, never as the chat's clone has them, and marks its head as viewed
  only when you were shown the whole diff. On a forge purlis can't read a diff from yet, such as
  GitLab, View changes says so and marks nothing. The row says the head is viewed, or which
  person other than its author approved this head on GitHub, or how many commits came since you
  looked. A head the chat didn't push, by the commits purlis pushed for it, is flagged. While a
  merge waits, purlis watches the request: a head that moves, or checks or an approval that
  change, redraw its row, which asks again. A merge still takes two presses, whatever the row
  says. What was viewed is kept only while the app runs (#6432).

### Changed

- **Merge merges only what its row showed.** A press now hands back every fact its row showed,
  so checks, a title or an approval that changed after Merge asked once more ask again, as a
  changed ruleset already did (#6432).
