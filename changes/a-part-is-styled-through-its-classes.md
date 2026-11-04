### Changed

- **The window restyles itself faster.** Each component's rules are now keyed on classes rather
  than attributes, which roughly halves the style work when the window restyles itself, as it
  does when Skill studio opens from the palette (#4351).
