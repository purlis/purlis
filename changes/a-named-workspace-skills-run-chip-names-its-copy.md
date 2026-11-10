### Changed

- A run of another workspace's skill that a persona names (`workspaces/ops:deploy`) now names that copy: the chat's run chip says "deploy of workspace ops" and opens `ops`'s runs, and the event log's `skill.started` says `skill_copy_workspace`. A LOCAL workspace's name is still never written (#6083).
