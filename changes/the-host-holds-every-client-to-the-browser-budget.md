### Fixed

- **The browser budget holds for every client of the session host** (#2611). The most browsers
  your device runs at once (Settings › This machine › Browsers) is now kept by the session host
  itself, whoever asks it for a browser, so a second window or a runner's host never starts more
  than its device's budget, and two browsers started at the same moment never both take the last
  place. A browser refused for it says so, as before.
