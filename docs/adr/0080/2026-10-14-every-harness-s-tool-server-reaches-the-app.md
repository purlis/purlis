# Amended 2026-10-14: every harness's tool server reaches the app on the chat's own socket and token (#1996)

Ruled 2026-10-14 (round 2 of the browser-flow grill): *"Every harness's tool server gets the
chat's socket and token; browser tools ride the hook socket on a framed byte stream."*

- **The chat's existing token and hook socket reach every harness's tool server**, the ACP
  agent's included. The server is handed the four variables that connect the chat's hooks to the
  app (`PURLIS_HOOK_SOCKET`, `PURLIS_CHAT`, `PURLIS_CHAT_TOKEN`, `PURLIS_HARNESS`;
  `chattools::CONNECTION_ENV`) beside the chat's place. It is the token minted at the chat's
  `exec` for its hooks: nothing is minted for the server, no scope is widened, and no value is
  written to a file or a command line. Claude Code and opencode hand it on in the environment
  they give a stdio server; Codex by the names in `env_vars`; an ACP agent in `session/new`'s
  `mcpServers[].env`, over its own stdio, from the environment it already runs in.
- **The hook socket carries a framed byte stream beside its lines**, chosen by the client with a
  hello line and accepted by the app on one line; an app from before it is asked again in lines.
  Every framed ask is admitted as a line is (the token, then the sending process), so an answer
  may be larger than a line and may carry bytes without a second way in. The wire is recorded in
  `docs/plane-format.md` (`app/hooks.sock`).
- **To be confirmed** (D-1996, the implementation's choices, recorded in PR #2067): the hello as a
  line of its own kind rather than a preamble or a second socket; a frame as a four-byte
  big-endian length and a kind byte (JSON or bytes), a reply as a JSON head and one bytes frame
  per attachment; Codex given names, never values; an ask's frame held to a line's bound and a
  reply's frames to 16 MiB; an attachment that is not an image passed to MCP as an embedded blob
  resource; `persona_where` the first tool to ask over frames.
