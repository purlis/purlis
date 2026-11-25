### Fixed

- **`gh auth status` and issue search work in a chat on the forge route.** `gh auth status` read
  the API root, which the route refused, so it called the token invalid while every other `gh`
  read worked; the root is answered now, by the forge for the chat's own token. `gh issue list
  --search` and the REST `GET /search/issues?q=` are answered when the query names exactly one
  `repo:`, the chat's own, so a chat can look for an existing issue before filing one; a search of
  another repo, of none, of two, or one with `org:`, `user:` or `OR` is still refused. The advice
  about `gh pr create` and `gh pr update-branch` is now said only on those two mutations, not on
  every refusal of another repo (#7544).
