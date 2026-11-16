### Security

- **A chat already running when the first model key is added restarts at its next pause.** Its
  sandbox was set up before the key existed, so it still reached the system's credential service.
  purlis now owes every such sandboxed chat one restart, made when its turn ends and never in the
  middle of one, and its pane says why, with *Restart now*. A chat purlis takes back when it
  opens is judged the same way, against the keys it finds. Until that pause, the chat keeps the
  sandbox it started with (T1a-78, #6921).
