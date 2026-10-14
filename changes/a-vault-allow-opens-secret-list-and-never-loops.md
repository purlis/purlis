### Fixed

- **A sandboxed chat lists a vault's key names through the app.** `purlis secret list <vault>`
  in a sandboxed chat is answered by the app for a vault the chat's persona may use, so after
  you press Allow on a vault's notice the chat's listing works instead of failing on the vault
  file. The chat is told exactly what the Allow opened: the key listing and the persona's
  declared tools. `secret get` and `secret cp` stay refused in a sandboxed chat, with a sentence
  naming the route that works (qw385).
- **A vault notice no longer loops.** One Allow answers every chat of the same persona refused
  the same vault; pressing Allow where the vault is already allowed says so and closes the
  notice; Dispatch closes it too (qw385).
