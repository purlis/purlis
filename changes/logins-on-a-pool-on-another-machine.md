### Added

- **Saved logins reach a pool on another machine once you say only you sign in there.** Connect
  ends by asking *Who can sign in to {host}?*, with *Others might, or I'm not sure* recommended,
  and the pool's row in Settings › Pools asks it again. Choose *Only me, and people I trust with my
  logins*, and that pool's chats may use your saved logins in a browser while this machine is
  connected, and a tool's value for one run, each only when you allow it. If the machine's key,
  certificate authority or Docker changes, the answer goes back to not sure and each login asks
  again. Until then its runner is sent nothing, and the chat says why (#6271, ADR 0123 §5).
