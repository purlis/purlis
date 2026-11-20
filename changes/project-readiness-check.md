### Added

- **purlis checks whether a project is ready for everyone who has access to it.** `purlis doctor`
  has a `project readiness` row, and Settings › You has a Ready for all users group (with a
  project open). Each reads the forge as you and says, per check, Ready, Blocked (and who can fix
  it: you, the owner or the administrator) or Not read. A setting purlis cannot read counts as
  not ready. The checks are your role in the repo, your organization's approval of purlis's
  GitHub OAuth app, purlis's OAuth app on a self-hosted GitHub or GitLab server, whether the
  server lets users register OAuth applications (GitLab), the default branch's protection, and,
  where a token is used in CI, whether it may push. Each blocker has the clicks with their
  addresses and, when you are not the one who can fix it, a request to send them.
  Only purlis's own sign-in can show an organization's approval as ready: read over the `gh` or
  `glab` login, it is Not read. A 403 that names no cause, or SAML, is guided and never ready, and
  `purlis doctor` says "not checked" inside a chat, and where there is no origin.
