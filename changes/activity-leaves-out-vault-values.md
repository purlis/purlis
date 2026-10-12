### Security

- **Activity leaves out a message that holds a vault value, whatever it looks like.** Before,
  only text shaped like a credential was left out. A value purlis read from a vault or stored in
  one since it started, or a forge token it holds, now keeps the message's line in Activity and
  says its text was left out (#1520).
