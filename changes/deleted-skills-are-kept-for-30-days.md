### Changed

- **A skill deleted on this machine is kept for 30 days, then removed** (Skill studio preview,
  #3004). A skill whose delete could not be saved as a commit, a local one for example, is kept on
  this machine so Restore can bring it back. Those copies used to pile up. Now each stays 30 days
  after its delete and is then removed, as the Skill studio opens or before a `purlis skill`
  change. Each row of Deleted skills, and `purlis skill restore`, says how many days it has left.
  A skill whose delete was saved stays in the project's history, as before.
