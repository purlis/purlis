### Fixed

- **`gh run view --json jobs`, a filtered `gh pr list` and Dependabot alerts work in a chat on the
  forge route.** gh follows a run's `jobs_url`, an `https://api.github.com/...` address, and sends
  it on the chat's socket with that `Host` and no port; the route read the port as 80, so it refused
  every such request as `MalformedRequest`. It reads 443 now for one of the chat's GitHub API hosts,
  and a request still refused as malformed is told which check failed. A GraphQL search (`gh pr list
  --author ...`) is answered when every `repo:` in it is one of the chat's repos, named once or
  more, and a value gh quotes (`label:"needs review"`) is read; a quoted repo, `org:`, `user:`,
  another repo or none is still refused. `GET` of the chat's own repo's Dependabot alerts, the list
  and one alert, is answered; dismissing one is still refused (#7679, #7680, #7681).
