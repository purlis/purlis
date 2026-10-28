### Added

- **A skill router that reads prompts suggests skills on each prompt** (Skill studio preview,
  #3231). Where a chat's chosen router extension says in its approval that it reads prompts, each
  prompt is offered to it with the skills the chat's list leaves out, and the ones it picks reach
  that prompt as a short note naming each with purlis's own description and saying `skill_load`
  loads it. The router has 100 ms; past that the prompt goes on without the note. No other chat's
  prompt is handed to a router.
