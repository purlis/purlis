### Fixed

- **A plane's root is never left part-way by a save.** Once a merged request's newer saves are
  settled, they go onto the target as one commit made off to the side, and the plane moves onto
  it in one step. purlis no longer rebases them in place, so a conflict in a generated file no
  longer blocks saving, and a quit no longer cuts a rebase off. A rebase purlis started and a
  quit or crash cut off is recovered by the window's next look or save, with everything chats
  wrote since kept. A stop you made yourself stays yours. Quitting while a save writes the
  plane's git waits for it, and never ends it part-way (#5824).
