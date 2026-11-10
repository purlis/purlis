### Added

- **Build an artifact page outside purlis, and check it before anyone approves it.**
  `purlis artifact types` prints `purlis-page.d.ts`, the types of `purlis.port` generated from
  the query catalogue: what a page asks, and each query's parameters and data.
  `purlis artifact mock` prints a mock of the port that answers from `purlis-fixtures.json`
  beside the page, in the same envelope and under the same declaration, so a page runs in any
  browser; in purlis it does nothing. `purlis artifact check <folder>` refuses a declaration
  purlis would not read (an unknown query, a version or parameter it does not answer) and any
  query the page's code calls without declaring it. None of the three needs a project (#5975).
