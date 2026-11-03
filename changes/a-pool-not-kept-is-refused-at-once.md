### Changed

- **A chat's dispatch to a pool this machine doesn't keep is refused at once.** It used to wait
  for your Approve, though no start could run it. Now nothing is held, and the chat is told
  that the pool isn't one this machine keeps (#4965).
