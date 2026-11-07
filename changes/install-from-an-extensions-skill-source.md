### Added

- **A skill from an extension's skill source installs and updates like one from git** (#3089,
  behind the `skill-studio` flag). The install copies the files you reviewed, never asking the
  extension again, and pins the copy to the extension's revision in `skills.lock`. A file that
  holds such a pin is version 2, which an older purlis refuses whole. `purlis skill update`
  lists and updates these skills too. An answer naming two paths that differ only by case is
  refused before the review.
