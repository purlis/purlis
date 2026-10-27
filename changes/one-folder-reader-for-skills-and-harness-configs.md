### Changed

- **A skills folder of more than 2000 names is not read, and says so.** A plane level holding more
  names than purlis reads of one folder lists none of its skills, and names the folder once with
  the reason, as a clone's skills folder and a project's opencode plugin folder already did
  (SK-52, #3205).
- **A project's opencode config with a second name is not read.** An `opencode.json` or
  `opencode.jsonc` inside the project that is a hard link is listed as a config purlis cannot
  read, as a skill's own file with a second name is. The home folder's and the folders above
  the project still read one (SK-52, #3205).
