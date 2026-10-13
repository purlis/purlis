### Changed

- **A link's calls are answered side by side, and a client's event queue is bounded.** The
  session protocol's host answers each call on a task of its own, so a command that waits (a
  `start` that waits on a harness, or one of the window's own commands) holds back neither
  another command's answer nor a pushed event on the same link. The commands about one chat are
  still carried out in the order they came, so keystrokes never overtake each other, and a link
  has at most 32 calls in flight. A client that subscribes and stops reading now holds at most
  1024 events: past that it lets go of the subscription, and when it reads again it is told the
  cursor to subscribe from, which brings back what it did not keep. Nothing on the wire changed
  (#1081, #1095).
