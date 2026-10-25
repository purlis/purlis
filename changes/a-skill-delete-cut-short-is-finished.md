### Fixed

- **A skill delete cut short by a crash or a quit is finished** (Skill studio preview, #3013).
  If purlis stopped after deleting a committed skill and before saving that, the skill was gone
  from the Skill studio and from Restore until the next save, and every later delete was kept on
  this machine instead of saved. Now the Skill studio, or the next `purlis skill` change, finishes
  the delete in one commit, so Restore brings it back from there. If the project cannot save it
  then, the skill is put back. A Notice says which, and if neither works it says so as trouble
  and tries again the next time.
