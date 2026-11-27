### Changed

- **A chat you start waits while the project's policy needs your answer.** While this machine's
  acceptance of a project's policy is broken, or the committed policy waits for your Accept, a
  chat you start in that project (from the window, a resumed session, a reopened task or a pool
  chat) waits until you answer; Cancel stops it. "Work in strict mode" or "Keep my current rules"
  is kept for this run only, and a chat restored at open, a task a chat dispatches and a chat's
  handoff start at once under the policy in force (ADR 0130, #7724).
