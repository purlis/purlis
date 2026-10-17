### Added

- **A chat's browser can ask you to take over.** When a page needs a person (a CAPTCHA, a second
  factor, a consent page), the agent asks through `ask_operator` pointed at its browser, and an
  Ask shows under Needs you. Take over opens the browser with you driving; Done hands it back and
  tells the chat only that you finished, never what you did. If the browser ends first, the Ask
  ends with it and the Inbox keeps an update (#2015).
