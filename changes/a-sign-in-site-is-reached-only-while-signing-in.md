### Changed

- **A browser login that signs in on another host reaches that host only while it signs in.**
  Once the browser is back on the login's page, it reaches the page's site alone; a site that
  never sends it back stops being reached after two minutes. A later login goes through the same
  steps again.
