### Added

- **A chat on a pool's runner has its browser there too.** Its browser runs on the runner beside
  it, so its `localhost` is the runner's, and the browser's chip says so: `localhost:5173 on pool
  builds`. You watch it, take it over and type into it as on this machine. Over the link a view
  is held to 15 frames a second and 1280 pixels wide, sends the newest frame once the last is
  drawn, and never queues what you type behind its frames (#2050).
- **A login in a runner's browser asks first, and only on a runner that is yours.** For now that
  is a Docker pool on this machine's own Docker; a pool on another machine's engine is filled no
  login. The first time a runner chat asks purlis to log its browser in, you are asked about that
  login on that pool; your answer holds while purlis runs and the pool, its runner and its image
  stay as they are. The values go from your vault to the runner's browser for that one login; the
  runner keeps none, and no kept login. With your desktop away, the login waits up to 15 minutes
  for you to be asked, and the chat carries on (#2050).
