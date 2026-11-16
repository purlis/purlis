### Security

- **A model key in your keys keeps every chat off the system's keychain.** With one key in your
  keys, every chat's sandbox denies the credential service as a keyring vault makes it do, so a
  session bus you allowed a chat on Linux is no longer given to it, and no command it runs can ask
  the Secret Service for your model keys. `purlis secret exec`, `secret_ask`, the browser's secret
  fill and a tool value sent to a runner each refuse a your-keys entry by name (T1a-72, #6764).
