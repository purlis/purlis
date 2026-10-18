# Amended 2026-10-14 (the operator's ruling on spec #2008, built in #2560): a chat without the sandbox gets its persona's browser logins

Under the ruling of 2026-10-14 (a chat started without the sandbox gets browsers, logins and vault
values like a sandboxed one), item 2 of the amendment *a browser login is performed in the chat's
own browser* is read as follows. Where it disagrees, this note holds.

1. **The same login, held to the same rules.** A chat started without the sandbox may name one of
   its persona's declared browser logins to `browser_login`, and purlis performs it as it does for
   a sandboxed chat: a login its persona declares, allowed on this machine with the persona's
   hosts and tools under policy, from a vault the registry, or your grant on this machine, lets
   that persona use. Anything else is refused with a sentence, and nothing is filled.
2. **Read at the call.** No start armed such a chat's tools, so its persona's declarations and this
   machine's Allow are read when it asks, from the project's committed settings, whether or not
   the project turned the sandbox on, and from where no chat writes. An Allow given since applies
   to its next call; one withdrawn since refuses it. *Decided in implementation, D-2560-1, to be
   confirmed.*
