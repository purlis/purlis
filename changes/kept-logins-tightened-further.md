### Security

- **Kept browser logins are tightened further** (#2693). A kept login is opened under the same
  lock as its write-back, so a browser opened at that moment is never started fresh by mistake.
  A browser that outlived the removal of its profile never makes a store for that profile again.
  Only a browser that did nothing before its first login, or one opened from that login's store,
  is kept, and the chat is told to log in from a new browser otherwise. A build that does not
  say what of a browser's folder to leave out keeps no login.
