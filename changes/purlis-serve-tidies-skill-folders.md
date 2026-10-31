### Fixed

- **`purlis serve` tidies the skills folders it lays out** (Skill studio preview, #2879). A
  runner, a Docker host or a Kubernetes pod lays out a folder of skills for each chat it starts,
  and never removed them. Now it removes those no running chat uses as it starts and after each
  chat it hosts ends, as the app does.
