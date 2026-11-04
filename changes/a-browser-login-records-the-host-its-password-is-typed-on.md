### Changed

- **A browser login follows the site to its login form before you're asked** (#4995). On a site
  that signs in on another host, the notice shows the page it opens and the host the password is
  typed on, both large, and warns when they differ. The password is typed on that host alone,
  and Always allow records it in the login's line as `form`.
