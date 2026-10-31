### Fixed

- **A skill that refers to a built-in skill installs** (Skill studio preview, #4155). Where a skill
  refers to a built-in skill such as `secrets`, and its source also holds a folder of that name,
  the install brought that folder along and then refused, as no skill may take a built-in's name.
  The reference now resolves to purlis's built-in skill: nothing is brought along, the review
  says it is left as it is, and an update that newly refers to one goes ahead.
