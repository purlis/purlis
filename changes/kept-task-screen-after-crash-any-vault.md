### Fixed

- After a crash, a task that used a 1Password or reference vault, a one-time code or a GitHub
  App token keeps its last screen, masked, instead of losing it. purlis now finds each value it
  handed a chat by a print and a seal it wrote as it handed it, so the launch reads no vault and
  asks you nothing (#6907).
