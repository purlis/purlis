### Added

- **A branch's check results on its diff.** `purlis check report <branch> <file.sarif>` hands purlis
  a tool's results in SARIF 2.1.0, from a script, an agent's hook or CI. "What changed" in a file then
  marks them in the gutter on the changed lines and lists each with its tool, severity and message,
  those outside the changed lines and in other files too. Each opens at its line, in your editor as
  well, and they are marked out of date once the branch's head moves (#5407).
