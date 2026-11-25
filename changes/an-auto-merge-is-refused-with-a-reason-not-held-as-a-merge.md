### Fixed

- **A merge that asks the forge to merge later is refused with a reason, not held as a Merge ask.**
  `gh pr merge --auto`, `glab mr merge --auto-merge` (or `--when-pipeline-succeeds`), and a GitLab
  REST merge whose body sets `merge_when_pipeline_succeeds` or `auto_merge` to true used to ask you
  to merge, and your press merged at once, not later. They are now refused, nobody is asked, and
  the chat is told to ask you to merge it or to run the merge without the auto flag (#7597).
