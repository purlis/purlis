### Fixed

- **A browser's start no longer fails on one bad answer from a server.** When the browser
  engine's manifest or archive answers with a server error, such as a 504 from GitHub, purlis
  asks again twice, a second and then two seconds later, before it gives up. Until now one such
  answer could refuse an outdated engine that a newer one would have replaced.
