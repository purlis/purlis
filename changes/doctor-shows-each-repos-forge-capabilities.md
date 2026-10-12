### Added

- **`purlis doctor` shows what each workspace repo's forge can do.** A new `forge capabilities`
  row lists, for every clone whose origin is on GitHub or GitLab, each capability purlis uses
  (auto-merge, merge queue, sub-issues, issue types, epics, iterations, boards, dependencies,
  close reasons, item status): yes, no with the reason (a personal account, a GitLab tier, an
  older host), or unknown, and what purlis does instead. The answers come from the same check
  the operations make. Only a doctor you type asks the forge; the preflight does not (#802).
