### Security

- **A page holding a filled secret asks before its server's redirect takes it to a site you
  allowed for something else.** Only your answer to the ask about that page leaving counts: an
  earlier Allow of the same site, when the browser asked to go there for another reason, no
  longer lets the redirect go by itself. Each browser asks at least once.
