### Changed

- **A run's notice offers Allow for an ssh jump to a scoped link-local host** written as ssh
  reads it in a jump, `-J u@[fe80::1%%en0]`, as the last hop. Any other `%` in a jump still gets
  no Allow, since ssh expands its `%` tokens in the command its shell runs (#5893).
