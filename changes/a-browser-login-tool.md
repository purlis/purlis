### Added

- **A browser login is a persona's tool.** `[sandbox.personas.<persona>] tools = { shop-login =
  { vault, browser = "https://…/login", user, key } }` lets a sandboxed chat log a browser in:
  it runs `shop-login -s=<session>`, and purlis opens the declared page in a browser of its own,
  fills the login from the vault only on that page's site, and leaves the logged-in session for
  the chat to load. The chat never sees the login and cannot send it to another page, host or
  program. It waits for the same Allow as the persona's hosts and tools, and needs Playwright's
  `playwright-cli` installed (#1407).
