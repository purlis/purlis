### Fixed

- A harness profile can no longer be named after any `purlis` command: the names a profile may
  not take are now the command line's own words, checked against its parser, so `purlis serve`,
  `purlis profile` and every command added later are refused as profile names like the rest.
  A profile you already have under one of these names is now listed as refused: rename its
  `[harness.<name>]` table (#2422).
