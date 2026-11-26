### Security

- **A run's notice offers no Allow for daemontools' `svscan`, `supervise` or `multilog`, or for
  its `setuidgid`, `envuidgid`, `softlimit`, `setlock`, `pgrphack` and `fghack`**. `supervise`
  runs a folder's `run` script and `svscan` starts one on each folder it finds, `multilog` has
  a shell run the processor it is named, and the rest run the program after them. A persona can
  still declare any of them as a tool (#6559).
