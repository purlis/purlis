# Amended (2026-10-14, qw381; the operator's ruling T-3): a persona's MCP server's start is tried twice

A persona's credentialed MCP server is started by the chat's harness, as `purlis secret exec
<vault> … -- <server>`, which asks the app for the server's run (section 5, class 1). Two things
are added to that start.

- **The app waits for a chat that is still starting.** A harness starts its MCP servers as soon
  as its program runs, and the app records the chat as open only once its start is done. A
  brokered run asked for by a line that carried the chat's current token, from inside its
  program, of a chat the app has begun to start, now waits up to ten seconds for that record
  before it is refused, and the record is used only while the chat's program is the one
  recorded when the wait began. The authorisation, the persona and the sandbox the run is held
  to are still the app's record of the chat, read once it exists.
- **One more try, and then the person is told.** purlis marks the start it wraps with the
  server's name (`PURLIS_MCP_SERVER`, in the server's environment). A marked `secret exec` whose
  run ended before the server wrote on stdout, or was refused because the chat was not open yet,
  asks once more after two seconds, handing the second run the stdin the first one took; the
  second ask says it is the last. A refusal that is the same on every ask is not asked again,
  and once a run has taken the stdin nothing runs in the chat in its place. A server that wrote
  anything is never started again by purlis. When the start fails for good, the app keeps the
  failure for the chat's tab, where a Notice names the server and the error its process gave
  (its last stderr, bounded, masked with the run's values and purlis's credential shapes) and
  offers Retry, the chat's restart on its conversation; and the chat is told at its next turn,
  in fixed words with the error quoted as data. A chat whose start outlasts the wait is told
  the same. The app hears a marked start only within a minute of the chat's own start, and
  names a server only where the chat's persona, as the app started it or is starting it,
  declares it; whether a run is a server's start, and the error, are the asking process's own
  words.

Only Claude Code is started with a persona's servers, so only there does any of this apply.
