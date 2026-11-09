### Fixed

- **A task's report reaches a busy app.** `purlis dispatch report`, `note` and `ask` used to
  give up after two seconds. When the window was busy, for example after an update had brought
  every chat back at once, a task was told no app was there, and its report went unsent. These
  commands now wait up to twenty seconds for an app that is there. Where the app still does not
  answer, the task is told so and to send its report again (#5845).
