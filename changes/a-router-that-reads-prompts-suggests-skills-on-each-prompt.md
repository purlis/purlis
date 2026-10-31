### Added

- **A skill router that reads prompts suggests skills on each prompt** (Skill studio preview,
  #3231). A router reads prompts only after you turn that on for it, on the Skill
  studio's Routing page, whose line says it reads every prompt; it is off by default. Then each
  prompt of a chat it routes is offered to it with the skills the chat's list leaves out, and the ones it picks reach
  that prompt as a short note naming each with purlis's own description and saying `skill_load`
  loads it. The router stays confined and has 100 ms; past that the prompt goes on without the note and
  the routing record says it fell back. purlis writes no prompt down, and no other chat's prompt
  is handed to a router.
