### Security

- **A declared tool runs only where the repository's settings are yours.** A tool run with a
  vault's value from a chat now starts only in a folder whose repository keeps its git settings,
  the settings they include and its hooks where no chat can write them (your project's own
  clones and branch folders, or a folder outside any repository); glab also only where the
  repository holds no glab settings of its own. Elsewhere the run is refused with a sentence
  naming the file (#3868).
