### Changed

- **Every box you type in is one field family** (#2905). A name, a token, a filter, an answer
  and a list of entries are drawn and behave the same everywhere: a label, the box, a line of
  help and the reason a value is refused, as one. A field says what is wrong with a value when
  you leave it, and follows your typing until it is right; a form says it at each field and takes
  you to the first before it sends anything. **⌘⏎** (Ctrl+Enter off macOS) sends a form's main
  answer from anywhere in it, a box of several lines included, and starts a chat from Start a
  chat; it never presses an Allow or something that cannot be undone. A filter keeps your caret
  where it was while its list redraws.
