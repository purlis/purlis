### Fixed

- **Browsers start on Ubuntu 23.10 and later, their engine's sandbox on.** The engine purlis
  downloads needs an AppArmor profile there to make its sandbox, and without one every browser
  ended at its start. The `.deb` now installs that profile. Where purlis was installed another
  way, a Notice offers the command that adds it, or this machine's own Chrome for every browser,
  and the chat is told a browser is not available here instead of the engine's crash (#2835).
