### Added

- **purlis can install itself on another machine over SSH, and a new version never stops the
  old one.** The copy is checked against purlis's signed release before it can run there, and
  a copy that changed on the way is deleted and nothing is installed. It works whatever login
  shell the person uses there, fish included, and when that machine downloads the copy itself
  it uses curl over https only, redirects included. Each version keeps its own
  place on that machine, so chats already running there keep going on the old version while new
  chats use the new one. The machine's chats can't change what is installed. This is the
  install step of machines without Docker, which Pool setup offers once that kind is built
  (RR-14, #723).
