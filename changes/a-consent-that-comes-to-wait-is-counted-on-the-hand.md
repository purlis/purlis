### Fixed

- **A sandbox consent that comes to wait while the window is open is counted on the ✋.** When a
  project came to expect something this machine has not allowed, by a pull or an edit, its Notice
  appeared but Asks did not list it and the ✋ counted nothing until something else moved. Asks
  now reads its list again when the project's settings change on disk and when the window comes
  back into focus, as the Notice does (#6000).
