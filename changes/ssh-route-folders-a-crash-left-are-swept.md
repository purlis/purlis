### Fixed

- **The ssh route folders of chats an app crash left behind are removed.** Each route folder
  names the app process that made it, and opening the project removes those whose process is
  gone; a live chat's route, and anything purlis did not make, stay (#1708).
