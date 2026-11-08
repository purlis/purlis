### Fixed

- The Skill studio no longer says a skill's script calls a program because the word appears inside
  a quoted string or a here-document, such as `sort_by` in a jq program or `body{font:…` in CSS.
  A program the script really runs is still named, also inside `$(…)`, `bash -c '…'`, `eval` and
  a here-document fed to a shell.
