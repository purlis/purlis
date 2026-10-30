### Added

- **opencode's Copilot sign-in on a runner shows its device code in place.** Sign in to GitHub
  Copilot on a profile's line in Settings › Pools shows the code with Copy code and Open page,
  and the line turns signed in once GitHub accepts it. It signs in to GitHub.com; GitHub
  Enterprise stays with Open in terminal.
- **`purlis profile sign-in <profile> --pool <pool>`** signs a profile in on a pool's runner from
  your own terminal, with the same steps as the strip: it says the page and opens it, reads the
  code, or shows the device code, and says signed in. A refusal, or anything it does not recognise,
  says what to do and exits non-zero. So far it follows Claude Code and opencode's Copilot sign-in.
