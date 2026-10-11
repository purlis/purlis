### Changed

- **purlis asks a forge what it can do.** On GitHub it asks whether a repo's owner is an
  organisation: a personal account's repo has no issue types and no merge queue, and an
  organisation's repo on github.com has issue types. On gitlab.com it reads a group's plan,
  which GitLab shows to the group's owners. On a self-managed GitLab it reads the version and
  edition, and the licence if you are an administrator. A question nobody answered still takes
  its fallback (#802, #1202).
- **A GitLab auto-merge uses `auto_merge` from GitLab 17.11.** An older instance, or one that
  does not say its version, is still sent the deprecated `merge_when_pipeline_succeeds`, which
  it reads as "wait for the pipeline" (#805).
- **A self-managed GitLab shows a work item's own status.** This needs GitLab 18.1 and a Premium
  or Ultimate licence that your account can read. Elsewhere the item shows its board's status,
  as before (#1202).
