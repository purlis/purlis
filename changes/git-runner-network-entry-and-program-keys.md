### Security

- **The git purlis runs connects only through its network entry point.** A fetch, push, clone or
  other verb that can reach another repository is refused by every other runner before git
  starts, so none skips the one-credential rule or the network log. Two more config keys that
  start a program are held: a remote's helper (`remote.<name>.vcs`) and the commands a gc runs
  (`gc.recentObjectsHook`) (#1853).

### Fixed

- **The network log names where a git call really went.** A push that names no remote is listed
  at the remote git pushes to, and a URL is listed after git's rewrites, a push's own first
  (#1853, #1828).
