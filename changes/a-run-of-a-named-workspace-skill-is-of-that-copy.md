### Fixed

- **A run of another workspace's skill a persona names is that workspace's copy's.** Its record
  says whose copy it ran, and what it ran is read from that copy, never from a copy of the same
  name in the chat's own workspace. The skill's versions count it, and `skill_runs` tells it
  apart. With the Skill studio flag on, the studio's row of the copy edits, shares and shows the
  history of the copy in its own workspace (SK-5c, #6041).
