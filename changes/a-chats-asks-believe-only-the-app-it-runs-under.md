### Security

- **A chat can't fake or dodge the app's answer to its git and its guards.** When a chat's
  command asks the app something, such as whether a merge is held, whether a branch folder is
  another chat's, or to run its git, it now asks only the app the chat runs under, and the app
  works out which chat is asking from the processes themselves. A process in a chat could
  previously unset or change its environment so that its git ran without being judged, point it
  at a socket of its own that answered "allowed", or claim to be another chat. Now, if no app
  answers for a chat, the ask is refused rather than run in the chat (#6995).
