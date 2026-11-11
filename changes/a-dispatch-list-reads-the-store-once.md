### Fixed

- **A chat's `purlis dispatch list` answers sooner.** It read the dispatch records once for every
  open task and every record the project kept for its finished ones; it now reads them once, and
  of the finished ones only its own. A list a chat sends while its last one is still being
  answered shares that answer instead of reading again, and a slow list says in the log which
  step took the time.
