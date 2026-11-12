### Fixed

- **A chat can start an OAuth or OpenID Connect sign-in with its own request.** `browser_login`
  takes `start`, for a login your persona declares or one you allowed: an address on the login's
  page's site or the site it signs in on, such as an authorize address with its `client_id`,
  `redirect_uri`, `state` and PKCE challenge, opened exactly as written. Any `redirect_uri` must
  lead back to the login's page's site. The password is still filled by purlis alone, and only on
  the login form's site (#6291).
