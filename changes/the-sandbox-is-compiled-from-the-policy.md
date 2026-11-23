### Changed

- **A policy rule about files can no longer name `attended` or `harness_mode`, or use `like`.**
  A rule on `read` or `write` (or on every action) that does is refused when the policy loads,
  and the refusal names the rule and its line: a sandbox's profile is fixed at the chat's start
  and cannot follow either. Rules on other actions are not affected (M75-22, #7031).
