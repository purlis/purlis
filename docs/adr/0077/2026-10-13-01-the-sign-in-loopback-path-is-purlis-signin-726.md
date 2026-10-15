# Amended 2026-10-13: the sign-in loopback path is `/purlis/signin` (#726)

Amended 2026-10-13 (operator): the path is /purlis/signin; nothing was registered or listening
under the old path.

Every place this record spelled the path now says it: GitHub's authorization callback,
`http://127.0.0.1/purlis/signin`, and the PKCE redirect, whose three GitLab redirect URIs are
`http://127.0.0.1:47823/purlis/signin`, `http://127.0.0.1:47824/purlis/signin` and
`http://127.0.0.1:47825/purlis/signin`. Nothing else in the decision changes.
