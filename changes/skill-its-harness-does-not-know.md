### Fixed

- Chats are told to load a skill with `skill_load` when their program says it does not know that skill, so one skill naming another that was not handed to the program no longer has to stop at "Unknown skill".
- `skill_load` no longer starts a skill of this chat when asked for another plugin's skill of the same short name.
