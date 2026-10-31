### Added

- **`purlis persona lint` checks the skills a persona names** (Skill studio preview, #2934). Each
  entry of a persona's `skills:` list is read as a chat's start reads it, and lint warns, with the
  fix, of one that names nothing, one written with the old `charter:` prefix, a built-in skill
  purlis has none of, and a plugin skill or skill name that no skill the persona's chats reach and
  no harness on this machine has. Without the preview, lint says the line is read only with it on,
  where it used to say `skills:` was no longer read.
