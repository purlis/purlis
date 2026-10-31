### Fixed

- **A chat on a runner shows up in the Skill studio's runs, and gets the same skills** (Skill
  studio preview, #3084). A chat running on a Docker or Kubernetes pool logged its skill runs on
  the runner, where the studio never saw them, and with the preview on it could start without the
  project's skills. Its runs are now logged on your desktop with the persona, harness and
  workspace it ran in, and the runner gives it the same project skills a chat here gets.
