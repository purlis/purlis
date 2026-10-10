### Changed

- **One list: the Inbox holds what the Alerts drawer and the band under the tab strip held.**
  Every line that stood under the tab strip, two at a time, is now a Notice in the project's
  Inbox, after what asks for you and before what happened, every one listed with its ways out.
  The alerts are Notices there too: this project's and this machine's whole, and one line for
  each other open project that has some, which opens that project's Inbox. The status line's
  Alerts button is its Notices button, counting them and opening the Inbox. A Notice that
  answers what you just did (a refusal, an Undo, a saved record) or the summary of a time away
  brings the Inbox on screen (#1695).
- **The title bar's hand opens the Inbox, and drops no list of its own.** Where nothing waits in
  this window's projects but something waits in another window's, it brings that chat forward
  in its own window (#1695, #1700).

### Fixed

- **A reply from the Inbox is checked before it is sent.** If the chat no longer waits on a
  reply, nothing is typed into it and the Inbox says so. Home and End move the cursor in the
  reply box, a chat waiting on a reply that also has a reason to be listed gets the box, and the
  keyboard stays in the list after an answer takes its ask away (#1700).
