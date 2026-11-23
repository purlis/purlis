### Fixed

- A read of the forge whose endpoint holds an unquoted substitution of reads, such as `glab api projects/:id/pipelines/$(glab api projects/:id/pipelines | jq -r '.[0].id')/jobs`, is no longer held as a possible merge: it passes as the same call with the endpoint in double quotes does. A write, a merge, or a substitution that could print a flag stays held.
