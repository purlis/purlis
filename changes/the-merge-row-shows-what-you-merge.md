### Changed

- **A held merge's row shows what you would merge, as the forge says it now.** In Asks and on
  the chat's tab it names the repo and the head, says there is no undo, shows the branch it
  merges into, who opened it, the checks as of the forge's read, and whether a ruleset protects
  the branch, beside the gap: a script that merges with the chat's token gets past purlis. The
  request's title is shown last, as text the chat may have written. Merge reads the request
  again before it asks once more, and the press merges only while the ruleset state you were
  shown is the one purlis reads; a refusal of the forge's is said on the row (#6431).

### Fixed

- **A held merge can no longer be answered by a double click or a press on a row just drawn.**
  On the chat's tab, the Merge that asks once more waits a moment before it acts, and in Asks,
  Don't merge waits for the row to settle as Merge does. A press at a head the chat moved past
  while it was being merged no longer leaves a second row for the same request (#6431).
