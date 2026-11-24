### Changed

- **A policy rule about files can no longer name `attended` or `harness_mode`, or use `like`.**
  A rule on `read` or `write` (or on every action) that does, or that no profile can follow, is
  refused when the policy loads, and the refusal names the rule and its line: a sandbox's profile
  is fixed at the chat's start and cannot follow either. A path written with `..`, `.`, a doubled
  or trailing slash, or not absolute, in a rule on any action, is refused the same way (M75-22,
  #7031).
- **A rule about a path now holds the same under the hook as in the sandbox.** The hook read a
  folder's contents, a named set's members and a workspace's files differently from the policy's
  own reading; one resolver now serves both, so a forbid on a folder holds for the files in it
  (M75-22, #7031).
