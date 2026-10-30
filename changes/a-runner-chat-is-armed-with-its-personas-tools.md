### Fixed

- **A persona's tools now work in a chat on a runner.** A runner never had your Allow of a
  persona's tools, so it armed none of them and their commands were refused there. A chat's start
  now carries what you allowed on your desktop, and the runner arms those tools while its copy of
  the persona matches. It never gets the persona's hosts this way, and every run still asks you
  (#4071).
