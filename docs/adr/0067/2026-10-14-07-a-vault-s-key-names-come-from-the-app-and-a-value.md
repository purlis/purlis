# Amended (2026-10-14, qw385; amends the 2026-10-10 paragraph on a refused vault's Allow): a vault's key names come from the app, and a value is never read in the chat — ruled by the operator, 2026-10-15

**Ruled by the operator, 2026-10-15.** Decided in implementation.

1. **`secret list <vault>` in a sandboxed chat is answered by the app** (and `persona secret
   list`). The chat sends one ask over its hook socket naming the vault and nothing else. The
   app authorises it exactly as it authorises a brokered `secret exec`: the persona it recorded
   for the chat, and the registry's tag or the person's grant on this machine. It answers the
   key names alone, sorted, and never a value. A vault the persona may not use is refused in
   the same words and raises the same Notice, so its Allow opens the same command. Where there
   is no app to ask (no socket, nothing listening), the command reads the vault where it runs,
   as before. Once the ask is sent, the app's answer is the only one: an app that does not
   answer in time, or one older than the ask, is said in a sentence, and the vault is not read
   in the chat. `vault list` is held to the same.
2. **A sandboxed chat never reads a value itself.** `secret get` and `secret cp` (and their
   `persona secret` forms) are refused in a sandboxed chat for every provider, whatever was
   allowed. The refusal names the routes that work: `secret list` for the key names, and
   `secret exec` through a tool the persona declares for a value (a terminal outside the chat
   where the app runs no command for a chat).
3. **The chat is told exactly what an Allow opens.** The line purlis sends after an Allow
   (ADR 0064's second gated send) names `secret list` of that vault, names `secret exec`
   through the persona's tools only where the persona declares one with the vault, and says a
   direct read of a value stays refused. It no longer says "run the
   command that was refused again". The Notice's answer says "The chat is told what now runs".
4. **A vault Notice never loops.** One Allow answers every chat opened as that persona that
   was refused the same vault, and each of them is told the same line, as its own board
   allows. A press where the persona may use the vault already says so,
   puts the Notice away, audits nothing and keeps nothing. After any press the window reads
   every chat's refusals of the project again, so a Notice whose refusal is gone is not drawn.
   Refusals are held in memory, so a relaunch draws none. Dispatch to {persona}… puts the
   Notice away as Keep blocked does; the chat's next try raises it again.

No CLI word and no key of the project's format was added.
