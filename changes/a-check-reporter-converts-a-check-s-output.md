### Added

- **A Check reporter turns a tool's output into SARIF.** An extension that declares the
  `check-reporter` capability, once you approve it, is handed a check's output and exit status by
  `purlis check report <branch> <output> --reporter <extension> --status <n>`, and the SARIF it
  answers is drawn on the branch's diff like any other check result. If what it answers is not
  SARIF, the review says so in a Notice naming the extension, and the results before stay. Extension
  protocol 5 carries the new request (#5410).
