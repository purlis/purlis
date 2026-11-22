### Security

- **A held CI-file push is read without writing your token to disk.** purlis fetches the base of a
  push it holds through its own git credential helper, which reads your sign-in from the keyring,
  instead of a file chats running as you could read. Quarantines a crash left are swept when
  purlis starts and when it quits.
- **A push is checked as the forge it is for.** A repo confirmed on GitHub is no longer taken for the
  repo of the same name on GitLab, so a CI-file push to the second is held for your answer. A repo
  confirmed on two forges is refused until purlis can tell which one a held act means.
- **A chat's forge connection proves itself both ways.** The door that terminates a chat's forge
  connections answers only that chat's own proxy, and the proxy carries bytes only once the door
  proves it is the door, so a reused local port never becomes a raw pipe.
