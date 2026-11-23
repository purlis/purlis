### Added

- **Windows Hello at each start, for the seal key** (Windows only, off by default). Settings ›
  Project › Sandbox › Sealed answers and device id now offers Turn on Windows Hello…. With it
  on, the key purlis seals your answers with is released only after you confirm with Windows
  Hello when purlis starts, so it no longer sits readable in the keyring. That narrows the gap and
  doesn't close it: a program you run can still read purlis's memory while it runs, or start a
  second purlis and be offered the same prompt, and Settings and doctor say so. If you cancel at
  the start, the key stays locked: on a project that already has sealed answers that is the deny
  state, and on a fresh install the shipped default applies. Turning it off asks Windows Hello
  again and is your press only. Windows Hello's signature is taken to be the same each time; if a
  device signs differently, turning it on says so and changes nothing (M75-43, #7052).
