### Changed

- **A chat on a pool's runner says every skill it started without, as a chat on this machine
  does, and says it again after the window restarts.** Its tab's Notice now also names skills
  that couldn't be loaded on the runner and skills the repo leaves out, each with why, and
  another workspace's skills its persona names that aren't there. A chat's runner on a k8s pool
  is named by its pool, not by an internal id. A runner on a purlis older than this one says only
  the skills it lacks, and only when the chat starts (#6078).
