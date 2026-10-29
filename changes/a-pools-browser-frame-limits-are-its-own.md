### Added

- **A pool sets how its runner's browsers are watched over the link.** Add a Docker pool has
  two more boxes: browser frames a second (1 to 60) and browser width in pixels (320 to 3840).
  Leave them empty to keep 15 and 1280. A change holds for the next browser opened there, and
  the runner doesn't have to be made again. A pool that sets either can't be read by an earlier
  purlis (#3297).
