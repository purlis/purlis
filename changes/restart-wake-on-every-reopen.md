### Fixed

- **Every reopen tells a chat what the restart ended, not only a quit.** A chat waiting on
  work of its own now gets its wake line when the session host crashed and Resume starts it
  again, whether it was idle or mid-turn, and when a chat the launch meant to reattach has to
  be started after all. The host-crash notice names what ended (for example "1 background
  shell") and no longer says "nothing was lost" for such a chat. An update, a window crash past
  its grace, Reopen all and Try to start again tell it as a quit does. A chat the session host
  kept running, and a chat Start fresh does not put back, are told nothing (#5752).
