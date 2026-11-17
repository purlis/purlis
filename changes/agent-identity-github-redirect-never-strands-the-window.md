### Fixed

- **Making a GitHub App for the agent no longer waits for good when GitHub's answer doesn't
  match.** If the browser comes back from GitHub with a code that isn't this try's, the step
  stops at once and says that GitHub made the App when you pressed Create GitHub App, where to
  find or delete it, and how to finish: paste the address your browser ended on, which works for
  an hour. A browser that sends many cookies to this machine gets through as well. The wait
  stops after 15 minutes either way and says so.
