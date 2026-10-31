### Added

- **A skill with no trigger tests is offered two ways to get some** (Skill studio preview, #3925).
  Its Grades tab now offers Write trigger tests, which starts the skill's `evals/triggers.json`
  from a short template (one request it should start for, one near miss it should leave alone),
  saves it as a change to the skill and opens it to write in; and Ask a chat to write them, which
  opens a chat on the project's default profile with a prompt typed and not sent, naming the
  skill, its file and the format. The chat's write comes back as a draft to accept.
