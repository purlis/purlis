# Amendment, 2026-11-21: Claude Code's web tools are judged at the full sandbox (T1.1-39, R-094, R-353)

The record says Claude Code's web tools are denied at the full sandbox. A deny rule takes the
tool out of the model's list before any check runs, so no hook and no `PermissionDenied` ever
heard of the call: the log stayed silent (the real Claude Code leg showed it, #6886). The person
ruled: judge `WebFetch`, allow `WebSearch`.

- **`WebFetch` is judged by the chat's proxy.** A `PreToolUse` hook, `pretooluse-web`, takes the
  host from the call's structured `url` and asks the chat's own proxy for a tunnel to it. The
  proxy decides by the host policy it holds for the chat, asks the person as it does for a
  command's connection, and records. A refusal is the hook's deny: a block, and a Notice. No
  second decision function.
- **It fails closed.** A missing or unparseable `url`, a scheme other than `http` and `https`, no
  proxy named for the chat, a proxy that does not answer, and a hook that dies each refuse the
  call, whatever the policy's mode.
- **A redirect to another host** comes back to the model as that URL, and its next call is judged
  as any other.
- **`WebSearch` is allowed and recorded.** It runs on Anthropic's side and reaches no host of the
  person's. A deny rule of the person's own still refuses it, since Claude Code applies those.
- **Basic is unchanged in effect**: the floor's proxy carries every host, so the judge lets every
  fetch through there and records it.
- **Other harnesses are as they were**: opencode's `webfetch` runs inside the wrap and the proxy
  holds it (N38), and Codex's live web search is off (N37).

The row "Sandbox: files and egress" reads, for Claude Code, "web tools judged by a hook at the
full sandbox (the chat's proxy decides the host) and allowed at Basic sandbox".
