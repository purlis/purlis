### Security

- **A model key in your keys denies every chat's sandbox the system's credential service.** With
  one key in your keys, a chat's sandbox denies the credential service as a keyring vault makes it
  do: on Linux a session bus you allowed a chat is no longer given to it, so no command it runs can
  ask the Secret Service for your model keys, and on macOS the full sandbox denies the security
  server. Not everything is kept off it: on macOS the basic sandbox, and a Claude Code chat's
  hooks and MCP servers, still reach the security server, where each item's own access rule (held
  to purlis's app) is what keeps them out. A chat already running when the first key is added keeps
  the sandbox it started with until it restarts. `purlis secret exec`, `secret_ask`, the browser's
  secret fill and a tool value sent to a runner each refuse a your-keys entry by name (T1a-72,
  #6764).
