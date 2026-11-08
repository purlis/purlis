### Added

- **Install from an extension's skill source on the command line, and update its skills together**
  (#5760, behind the `skill-studio` flag). `purlis skill install --extension <id>` lists, reviews
  and installs from the source an extension adds. It asks at your own terminal each time and has
  no `--yes`. A chat is refused, as for a git source. Review updates now takes an extension's
  skills beside git ones, each copied from what its own review read, in one commit with one Undo.
  An update that adds a script or other new capability still starts unticked. An extension's
  source page marks what each skill's files do, as a git source's does.
