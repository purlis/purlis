### Added

- **A chat's Chat log now lists what happened to it between its start and its end**: each time
  its sandbox was lifted for a run or put back on, and who did it; each ask, with its answer and
  who answered, on one row; each action purlis held for you, such as a merge only your Merge
  merges, marked where one of the chat's helpers was held; and each time it was paused for going
  round and round. Its allowed calls are one line above the rows ("212 allowed · 9 by your
  rules", the second count shown at 0 too), counting only the calls purlis allowed. A chat whose
  reports purlis cannot check, or that ran with No sandbox, says so first, if that was true at any
  start, or its sandbox was lifted for a run; a chat with no start in the log reads as unchecked.
  The event log now records an ask being raised, and who answered each ask, as it happens: loop
  and budget pauses (Raise the budget included), a harness's permission prompts (an answer given
  after the prompt timed out included), sandbox asks and held merges. Consent asks, a hook giving
  up, and answers given from `purlis inbox` are not recorded yet; they follow.
