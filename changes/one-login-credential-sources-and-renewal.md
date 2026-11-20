### Added

- **One login: purlis's own forge credential comes from one of two places, and renews itself.**
  A person who signed in in the window is renewed before their sign-in expires, without a prompt, and
  told once ("Sign in again to `<host>`") if the forge ends it. A CI job, `purlis serve` or a pool
  runner can be given a token without a person: `PURLIS_FORGE_TOKEN_<HOST>` (or `…_FILE`, for a
  mounted secret), or the forge CLI's own `GH_TOKEN`, `GITHUB_TOKEN`, `GITLAB_TOKEN` and
  `CI_JOB_TOKEN`. In the window the sign-in is the only source; headless, a token given for a host
  wins. `docs/credential-sources.md` has the names and the rule.
- **Sign in to a GitHub Enterprise Server or a self-hosted GitLab through an application you
  registered on it,** and have the sign-in renewed like github.com's. The client id is kept in
  `forge-registrations.json`.
