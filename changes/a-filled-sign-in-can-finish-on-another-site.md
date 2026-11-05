### Fixed

- **A sign-in whose server redirects to another site now finishes after `browser_fill_secret`.**
  A redirect from the approved site's own server, after a form sent there, takes the page on: to a
  site this browser was on before the fill by itself, and to any other once you allow it. The filled
  page is gone before anything reaches that site. Links, scripts and the chat's navigate stay
  refused, and a redirect whose address holds the value is refused. A page that ends on an error page
  no longer holds the browser (#5230).

### Changed

- **A filled form that posts straight to another site is refused unless the key is approved for that
  site too.** The chat's answer and the page you see in the browser name the site and say how to
  approve the key for it (#5230).
