### Fixed

- **Forgetting a pool always says which machine still holds its runner.** With a long machine
  alias or long runner names, the answer was cut short before it named the machine, so you
  weren't told where the runner's container and home volume stay, or given the commands that
  remove them there. Each name is still shown safely, and none purlis keeps is long enough to be
  cut (#7873).
