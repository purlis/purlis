### Security

- **The browser skill's terminal flow keeps Playwright's traces out of the project.** It points
  `playwright-cli`'s output at a private folder outside the project before the session opens,
  and closes the session and removes the folder when the flow ends, so a trace of a logged-in
  run is never left where a save could commit it (#2686).
