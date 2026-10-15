# Amended (2026-10-10, #1665): Claude Code chats through the same proxy

A Claude Code chat gets its own pair of ports from the same proxy, made when it starts and
closed when it ends, and its `--settings` names them as `sandbox.network.httpProxyPort` and
`socksProxyPort`. Section 3's "the harness's own proxy where it has one" no longer holds for a
Claude Code that takes them.

1. **Both ports, or neither.** Claude Code starts its own proxy for a port left out, and a
   command reaches whatever that one allows. purlis names them as one pair.
2. **From Claude Code 2.1.285**, the first that takes them. The version is read from the
   `--version` answer the start already asks for (ruling V87g). An older one, or an answer
   with no version, keeps Claude Code's own proxy and the allowed domains, as before; the
   first such chat on that version says so once, and its connections are not in the record.
3. **The allowed domains are still written.** Claude Code restricts a command's network only
   while they are set; with the ports set, its proxy step is purlis's, which decides by the
   same hosts and layers. A host a chat reached before is reached still, and none besides.
4. **No other local port.** Claude Code's sandbox lets a command connect to the two ports and no
   other loopback port, unless a settings source turns `allowLocalBinding` on, which opens
   every one to the chat. purlis's settings name it `false`, which
   outranks a user's or a project's setting. A Claude Code chat now binds no local port, as a
   wrapped chat binds none. An administrator's managed settings still outrank purlis's: where
   one turns `allowLocalBinding` on, `purlis doctor`'s `sandbox local ports` row names the file
   (#1699).
5. **Every connection is in the network record**, as for a wrapped chat, and a refusal raises
   its Block.
6. **The proxy alone.** A Claude Code chat's harness keeps its own temp folder and its git sets
   its own ssh through the SOCKS port, so purlis starts the chat's proxy and no temp directory
   beside it (#1699). Its ssh route is written where no chat writes, not in a temp directory
   (§ "tunnels", item 8, as amended 2026-10-12).
