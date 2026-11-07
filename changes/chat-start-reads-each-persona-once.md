### Fixed

- **A chat starts faster in a project with many personas.** Freezing each persona's tools at a
  chat's start read every persona's definition several times over; with 100 personas that was
  80 ms of every start. Each definition is now read once (#5486).
