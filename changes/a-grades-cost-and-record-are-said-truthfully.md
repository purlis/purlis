### Changed

- **A grade's cost and record are said truthfully** (#3273). The plan says which earlier grade
  its estimate comes from (its profile, its program and the day it ran) and prefers one on the
  profile you picked. A skill whose last grade's chats did not report their spend now reads as
  graded, with its cost not known. A row of the grade record that does not hold together, that
  is out of range or that is stamped ahead of your clock is skipped. The Grades panel, the plan
  and `purlis skill grade` say how many rows were skipped and name any record file too large to
  read.
