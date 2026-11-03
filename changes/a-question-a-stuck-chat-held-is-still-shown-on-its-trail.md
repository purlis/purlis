### Fixed

- **A task question's trail says when its asking chat saw it, even after that chat was stuck.**
  A question whose 100 seconds started while its asking chat sat on a prompt or a usage limit
  now still writes `question.shown` for the turn that has it, and its time is not started again
  (#4653).
