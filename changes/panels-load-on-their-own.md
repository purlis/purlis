### Changed

- **A project's slow panels load on their own.** While a project's save status, stale repo check
  or alerts are still being read, the title bar and the Inbox each say so with their own mark, and
  the chats are usable meanwhile. The alerts of every project are read once when a launch's restore
  settles, rather than again for each project that joins (#5325).
