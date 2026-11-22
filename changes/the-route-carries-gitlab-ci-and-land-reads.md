### Fixed

- **A chat's CI and landing work goes through the forge route on GitLab too.** On gitlab.com or a
  self-hosted GitLab, a chat can now read its project's protected branches, approval rules, a merge
  request's approval state and merge trains, download a job's log and artifacts, and retry a pipeline,
  all on its own project only. Retrying one job, cancelling, and starting a manual job (`play`) stay yours to do,
  as running gated code. Merges, tags, releases, project administration and every change to a rule
  stay refused.
