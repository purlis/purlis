### Changed

- **What a chat wrote reads as text.** In the Inbox, a chat's message, a task's question and the
  note a chat passed on with it, and in a pane, the report of a task that ended, show their bold,
  lists, code and tables instead of raw `**` and `-`, with their own line breaks. HTML in them is
  dropped, a picture is never loaded (its description shows instead), and a link shows the site
  it goes to and opens in your browser only when you press it. A message over six lines folds,
  with "Show all · N lines" to open it (#5214).
- **An Inbox card is quieter.** Its head is one line: what kind of ask it is, the chat, and what
  it waits on, how long and where, with Go to chat and a × that ignores it. A chat's words are a
  step smaller and quieter. Your reply is a strip at the card's foot, always there: it grows as
  you type, Enter sends, Shift+Enter starts a new line, and Esc leaves it (#5214).
