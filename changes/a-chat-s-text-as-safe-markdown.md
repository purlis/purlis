### Changed

- **What a chat wrote reads as text in the Inbox.** A chat's message, a task's question and the
  note a chat passed on with it show their bold, lists, code and tables instead of raw `**` and
  `-`, with their own line breaks. HTML in them is dropped, a picture is never loaded (its
  description shows instead), and a link shows the site it goes to and opens in your browser
  only when you press it. A message over six lines folds, with "Show all · N lines" to open it.
  On a card waiting on your reply, Send is the one button, and Go to chat and Ignore are small
  links under the reply box (#5214).
