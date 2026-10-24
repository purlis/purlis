### Added

- **A persona's own skills can be made, edited, renamed, moved, deleted and shared in the Skill studio preview** (#3003), from the studio opened on that persona.
  - **New skill** starts at the persona you came from.
  - **Moving** one of a persona's skills into the project's skills asks where it reaches, and adds it to the persona's skills, so the persona keeps it in every workspace. Moving a project or workspace skill to a persona takes it out of that persona's skills, and says which chats and personas no longer get it.
  - **Rename** also renames every persona's and skill's mention of it.
  - **Delete** says that the persona's chats use it, and Restore brings it back.
  - The same changes are on the command line, with `-p <persona>`, and `purlis skill mv --to-persona <persona>`.
- **A chat's write to a persona's skill becomes a draft** in the Skill studio, as at the project and workspace levels (#3048).
