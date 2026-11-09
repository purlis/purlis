### Changed

- **A `purlis` command that waits on you says where to answer.** `purlis secret ask`, a Secret
  ask from a brokered command, a `purlis secret exec` run that needs your Allow, and a runner
  chat's tool run now print "waiting for the person: Asks (✋) in the purlis window, project
  <name>, for up to N seconds". When the command stops waiting, it says the ask is still
  answerable there.
